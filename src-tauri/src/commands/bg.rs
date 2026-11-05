//! Background removal through Apple's Vision framework.
//!
//! `VNGenerateForegroundInstanceMaskRequest` (macOS 14+) segments the salient
//! subjects in a photo; the platform layer turns that mask into an RGBA image
//! with a transparent background. No model is shipped and no network is used —
//! the framework is part of macOS, so the app binary does not grow.
//!
//! Every other OS gets a clear “macOS only” error instead of a hidden failure.

use std::path::PathBuf;

use image::DynamicImage;
use tauri::{AppHandle, Runtime};

use super::{normalize_format, output_path, validate_input, SUFFIX_REMOVE_BG};

/// Formats that cannot store the resulting alpha channel are promoted to PNG.
fn alpha_format(requested: Option<String>, input: &std::path::Path) -> String {
    let format = match requested {
        Some(fmt) if !fmt.trim().is_empty() => normalize_format(&fmt),
        _ => {
            let ext = super::extension_of(input);
            if ext.is_empty() {
                "png".to_string()
            } else {
                normalize_format(&ext)
            }
        }
    };
    match format.as_str() {
        // JPEG/BMP/GIF/ICNS would silently flatten the cut-out back onto a colour.
        "jpeg" | "bmp" | "gif" | "icns" => "png".to_string(),
        other => other.to_string(),
    }
}

#[tauri::command(async)]
pub fn remove_background<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let img = segment(&input)?;
    let format = alpha_format(output_format, &input);
    let out = output_path(&input, SUFFIX_REMOVE_BG, &format)?;
    super::image::encode_any(&app, &img, &out, &format, None)?;

    Ok(format!(
        "Background removed: {} ({} x {}, {} bytes)",
        out.display(),
        img.width(),
        img.height(),
        std::fs::metadata(&out).map(|meta| meta.len()).unwrap_or(0)
    ))
}

/// True when the running macOS is new enough for the Vision request.
pub fn supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        platform::available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[tauri::command(async)]
pub fn background_removal_available() -> bool {
    supported()
}

#[cfg(target_os = "macos")]
fn segment(input: &std::path::Path) -> Result<DynamicImage, String> {
    platform::segmented_image(input)
}

#[cfg(not(target_os = "macos"))]
fn segment(_input: &std::path::Path) -> Result<DynamicImage, String> {
    Err(
        "Background removal uses Apple's Vision framework, so it only runs on macOS 14 or newer."
            .to_string(),
    )
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use image::{DynamicImage, ImageBuffer, RgbaImage};
    use objc2::AnyThread;
    use objc2_core_video::{
        kCVPixelFormatType_32ARGB, kCVPixelFormatType_32BGRA, kCVPixelFormatType_32RGBA,
        CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
        CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
        CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    };
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{VNGenerateForegroundInstanceMaskRequest, VNImageRequestHandler, VNRequest};

    /// macOS 14 (Sonoma) shipped the foreground instance mask request.
    const MIN_MAJOR: i32 = 14;

    pub fn available() -> bool {
        macos_major().map(|major| major >= MIN_MAJOR).unwrap_or(false)
    }

    fn macos_major() -> Option<i32> {
        // `NSProcessInfo.operatingSystemVersion` would need another framework
        // binding; the version plist is stable across releases.
        std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            .ok()
            .and_then(|plist| {
                let marker = "<key>ProductVersion</key>";
                let rest = plist.split(marker).nth(1)?;
                let start = rest.find("<string>")? + "<string>".len();
                let value = rest[start..].split("</string>").next()?;
                value.split('.').next()?.parse().ok()
            })
    }

    pub fn segmented_image(path: &Path) -> Result<DynamicImage, String> {
        if !available() {
            return Err("Background removal needs macOS 14 or newer (Apple Vision).".to_string());
        }

        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let options = NSDictionary::new();
        let handler = unsafe {
            VNImageRequestHandler::initWithURL_options(
                VNImageRequestHandler::alloc(),
                &url,
                &options,
            )
        };

        let request = unsafe { VNGenerateForegroundInstanceMaskRequest::new() };
        let requests = NSArray::from_slice(&[&*request as &VNRequest]);
        handler
            .performRequests_error(&requests)
            .map_err(|error| format!("Vision could not analyse {}: {}", path.display(), error))?;

        let results = unsafe { request.results() }
            .ok_or_else(|| format!("Vision returned no mask for {}.", path.display()))?;
        let observation = results.firstObject().ok_or_else(|| {
            format!(
                "No clear subject found in {} — background removal needs a photo with a \
                 recognisable foreground.",
                path.display()
            )
        })?;

        let masked = unsafe {
            observation.generateMaskedImageOfInstances_fromRequestHandler_croppedToInstancesExtent_error(
                &observation.allInstances(),
                &handler,
                false,
            )
        }
        .map_err(|error| format!("Vision could not cut out the background: {}", error))?;

        rgba_from_pixel_buffer(&masked)
    }

    /// Byte offsets of R, G, B and A inside one pixel, per CoreVideo's 32-bit
    /// formats (`'ARGB'`, `'BGRA'`, `'RGBA'`). Vision answers with `32BGRA` in
    /// practice, where the *last* byte holds alpha.
    fn channel_offsets(format: u32) -> Option<(usize, usize, usize, usize)> {
        if format == kCVPixelFormatType_32BGRA {
            Some((2, 1, 0, 3))
        } else if format == kCVPixelFormatType_32RGBA {
            Some((0, 1, 2, 3))
        } else if format == kCVPixelFormatType_32ARGB {
            Some((1, 2, 3, 0))
        } else {
            None
        }
    }

    fn rgba_from_pixel_buffer(buffer: &CVPixelBuffer) -> Result<DynamicImage, String> {
        let format = CVPixelBufferGetPixelFormatType(buffer);
        let width = CVPixelBufferGetWidth(buffer) as u32;
        let height = CVPixelBufferGetHeight(buffer) as u32;
        if width == 0 || height == 0 {
            return Err("Vision produced an empty image.".to_string());
        }

        let flags = CVPixelBufferLockFlags::ReadOnly;
        let status = unsafe { CVPixelBufferLockBaseAddress(buffer, flags) };
        if status != 0 {
            return Err(format!(
                "Could not read Vision's output buffer (status {}).",
                status
            ));
        }

        let stride = CVPixelBufferGetBytesPerRow(buffer);
        let base = CVPixelBufferGetBaseAddress(buffer) as *const u8;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let mut failure = None;

        match (base.is_null(), channel_offsets(format)) {
            (true, _) => failure = Some("Vision produced an empty buffer.".to_string()),
            (false, None) => {
                failure = Some(format!(
                    "Vision returned an unsupported pixel format ('{}').",
                    String::from_utf8_lossy(&format.to_be_bytes())
                ))
            }
            (false, Some((r, g, b, a))) => {
                for y in 0..height as usize {
                    let row = unsafe { base.add(y * stride) };
                    for x in 0..width as usize {
                        let px = unsafe { row.add(x * 4) };
                        let out = &mut pixels[(y * width as usize + x) * 4..][..4];
                        // `r/g/b/a` are source offsets; the destination is
                        // always RGBA. Vision keeps full colour under the alpha.
                        unsafe {
                            out[0] = *px.add(r);
                            out[1] = *px.add(g);
                            out[2] = *px.add(b);
                            out[3] = *px.add(a);
                        }
                    }
                }
            }
        }

        unsafe { CVPixelBufferUnlockBaseAddress(buffer, flags) };

        if let Some(message) = failure {
            return Err(message);
        }

        let image: RgbaImage = ImageBuffer::from_raw(width, height, pixels)
            .ok_or_else(|| "Vision returned a malformed image buffer.".to_string())?;
        Ok(DynamicImage::ImageRgba8(image))
    }
}
