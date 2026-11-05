//! Phase 1 — Image Studio backend (pure Rust, no external tools).
//!
//! * [`convert_image`] — decode anything the `image` crate understands and
//!   re-encode it in the requested format.
//! * [`compress_image`] — optional downscale + quality-based re-encode.
//! * [`edit_image`] — crop / rotate / flip / resize / filters / text.
//! * [`vectorize_image`] — raster → SVG (requires the `vectorize` feature).
//! * [`batch_convert`] / [`batch_compress`] — many files, progress events.
//!
//! HEIC and JXL are **not** compiled in by default: those inputs/outputs are
//! routed through ffmpeg (see `decode_via_ffmpeg` / `encode_via_ffmpeg`).
//! Enabling the `heic-native` / `jxl-native` features is left for a future pass.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use ab_glyph::{FontVec, PxScale};
use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, ImageReader};
use imageproc::drawing::draw_text_mut;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

use super::common::{run_tool_named, ProgressPayload};
use super::{
    extension_of, normalize_format, output_path, validate_input, SUFFIX_COMPRESS, SUFFIX_CONVERT,
    SUFFIX_EDIT, SUFFIX_VECTORIZE,
};

/// Formats this module can write natively.
pub const NATIVE_OUTPUT_FORMATS: &[&str] = &[
    "png", "jpeg", "webp", "gif", "bmp", "tiff", "ico", "icns", "qoi", "avif",
];

// --------------------------------------------------------------- conversion --

#[tauri::command(async)]
pub fn convert_image<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    quality: Option<u8>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let format = normalize_format(&output_format);
    let img = decode_any(&app, &input)?;
    let out = output_path(&input, SUFFIX_CONVERT, &format)?;
    encode_any(&app, &img, &out, &format, quality)?;

    Ok(format!("Converted: {} ({} bytes)", out.display(), size_of(&out)))
}

#[tauri::command(async)]
pub fn compress_image<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    quality: u8,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let format = normalize_format(&output_format);
    let img = decode_any(&app, &input)?;
    let img = maybe_downscale(img, max_width, max_height);
    let out = output_path(&input, SUFFIX_COMPRESS, &format)?;
    encode_any(&app, &img, &out, &format, Some(quality))?;

    Ok(format!(
        "Compressed: {} ({} x {}, {} bytes)",
        out.display(),
        img.width(),
        img.height(),
        size_of(&out)
    ))
}

/// Resize so the image fits inside `max_width` x `max_height`, keeping aspect.
fn maybe_downscale(
    img: DynamicImage,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let target_w = max_width.filter(|m| *m > 0).unwrap_or(w);
    let target_h = max_height.filter(|m| *m > 0).unwrap_or(h);
    if target_w >= w && target_h >= h {
        return img;
    }
    let scale = f64::min(target_w as f64 / w as f64, target_h as f64 / h as f64);
    let new_w = ((w as f64 * scale).round() as u32).max(1);
    let new_h = ((h as f64 * scale).round() as u32).max(1);
    img.resize(new_w, new_h, FilterType::Lanczos3)
}

// ------------------------------------------------------------------ editing --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropOp {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeOp {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub keep_aspect: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOp {
    pub content: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default = "default_font_size")]
    pub size: f32,
    #[serde(default = "default_text_color")]
    pub color: String,
    #[serde(default)]
    pub font_path: Option<String>,
}

fn default_font_size() -> f32 {
    32.0
}

fn default_text_color() -> String {
    "#ffffff".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditOps {
    #[serde(default)]
    pub crop: Option<CropOp>,
    /// 0 / 90 / 180 / 270, clockwise.
    #[serde(default)]
    pub rotate: Option<i32>,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    #[serde(default)]
    pub resize: Option<ResizeOp>,
    #[serde(default)]
    pub grayscale: bool,
    /// Gaussian blur sigma; `None` or `0` disables it.
    #[serde(default)]
    pub blur: Option<f32>,
    /// -100..=100
    #[serde(default)]
    pub brightness: Option<i32>,
    /// 1.0 is a no-op, values below 1 flatten, above 1 increase contrast.
    #[serde(default)]
    pub contrast: Option<f32>,
    #[serde(default)]
    pub sharpen: bool,
    #[serde(default)]
    pub text: Option<TextOp>,
}

/// Apply order is fixed: resize → crop → rotate → flip → filters → text.
#[tauri::command(async)]
pub fn edit_image<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    ops: ImageEditOps,
    output_format: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let format = match output_format {
        Some(fmt) if !fmt.trim().is_empty() => normalize_format(&fmt),
        _ => {
            let ext = extension_of(&input);
            if ext.is_empty() {
                "png".to_string()
            } else {
                normalize_format(&ext)
            }
        }
    };

    let mut img = decode_any(&app, &input)?;

    if let Some(resize) = &ops.resize {
        img = apply_resize(img, resize)?;
    }
    if let Some(crop) = &ops.crop {
        img = apply_crop(img, crop)?;
    }
    if let Some(angle) = ops.rotate {
        img = match ((angle % 360) + 360) % 360 {
            0 => img,
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            other => return Err(format!("Rotation must be 0/90/180/270, got {}", other)),
        };
    }
    if ops.flip_h {
        img = img.fliph();
    }
    if ops.flip_v {
        img = img.flipv();
    }
    if ops.grayscale {
        img = img.grayscale();
    }
    if let Some(sigma) = ops.blur.filter(|s| *s > 0.0) {
        img = img.blur(sigma);
    }
    if let Some(brightness) = ops.brightness.filter(|b| *b != 0) {
        img = img.brighten(brightness.clamp(-255, 255));
    }
    if let Some(contrast) = ops.contrast.filter(|c| (*c - 1.0).abs() > f32::EPSILON) {
        img = apply_contrast(img, contrast.clamp(0.0, 4.0));
    }
    if ops.sharpen {
        img = img.unsharpen(1.0, 1);
    }
    if let Some(text) = &ops.text {
        img = apply_text(img, text)?;
    }

    let out = output_path(&input, SUFFIX_EDIT, &format)?;
    encode_any(&app, &img, &out, &format, None)?;
    Ok(format!(
        "Edited: {} ({} x {})",
        out.display(),
        img.width(),
        img.height()
    ))
}

fn apply_resize(img: DynamicImage, resize: &ResizeOp) -> Result<DynamicImage, String> {
    if resize.width == 0 || resize.height == 0 {
        return Err("Resize width and height must be greater than zero.".to_string());
    }
    if resize.keep_aspect {
        let scale = f64::min(
            resize.width as f64 / img.width() as f64,
            resize.height as f64 / img.height() as f64,
        );
        let w = ((img.width() as f64 * scale).round() as u32).max(1);
        let h = ((img.height() as f64 * scale).round() as u32).max(1);
        Ok(img.resize(w, h, FilterType::Lanczos3))
    } else {
        Ok(img.resize_exact(
            resize.width,
            resize.height,
            FilterType::Lanczos3,
        ))
    }
}

fn apply_crop(img: DynamicImage, crop: &CropOp) -> Result<DynamicImage, String> {
    if crop.width == 0 || crop.height == 0 {
        return Err("Crop width and height must be greater than zero.".to_string());
    }
    if crop.x + crop.width > img.width() || crop.y + crop.height > img.height() {
        return Err(format!(
            "Crop rectangle {}x{}+{}+{} does not fit inside the {}x{} image.",
            crop.width,
            crop.height,
            crop.x,
            crop.y,
            img.width(),
            img.height()
        ));
    }
    Ok(img.crop_imm(crop.x, crop.y, crop.width, crop.height))
}

fn apply_contrast(img: DynamicImage, factor: f32) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    for pixel in rgba.pixels_mut() {
        for channel in 0..3 {
            let value = pixel[channel] as f32 / 255.0;
            let adjusted = ((value - 0.5) * factor + 0.5).clamp(0.0, 1.0);
            pixel[channel] = (adjusted * 255.0).round() as u8;
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn apply_text(img: DynamicImage, text: &TextOp) -> Result<DynamicImage, String> {
    if text.content.trim().is_empty() {
        return Ok(img);
    }
    let font_bytes = resolve_font(text.font_path.as_deref())?;
    let font = FontVec::try_from_vec(font_bytes)
        .map_err(|e| format!("Could not parse the font file: {}", e))?;
    let color = parse_color(&text.color)?;

    let mut canvas = img.to_rgba8();
    draw_text_mut(
        &mut canvas,
        color,
        text.x,
        text.y,
        PxScale::from(text.size.max(4.0)),
        &font,
        &text.content,
    );
    Ok(DynamicImage::ImageRgba8(canvas))
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` or a handful of colour names.
fn parse_color(value: &str) -> Result<image::Rgba<u8>, String> {
    let raw = value.trim().to_lowercase();
    let named = match raw.as_str() {
        "white" => Some([255, 255, 255, 255]),
        "black" => Some([0, 0, 0, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 200, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        _ => None,
    };
    if let Some([r, g, b, a]) = named {
        return Ok(image::Rgba([r, g, b, a]));
    }

    let hex = raw.trim_start_matches('#');
    let parse = |s: &str| u8::from_str_radix(s, 16);
    match hex.len() {
        3 => Ok(image::Rgba([
            parse(&hex[0..1]).map_err(bad_color)? * 17,
            parse(&hex[1..2]).map_err(bad_color)? * 17,
            parse(&hex[2..3]).map_err(bad_color)? * 17,
            255,
        ])),
        6 => Ok(image::Rgba([
            parse(&hex[0..2]).map_err(bad_color)?,
            parse(&hex[2..4]).map_err(bad_color)?,
            parse(&hex[4..6]).map_err(bad_color)?,
            255,
        ])),
        8 => Ok(image::Rgba([
            parse(&hex[0..2]).map_err(bad_color)?,
            parse(&hex[2..4]).map_err(bad_color)?,
            parse(&hex[4..6]).map_err(bad_color)?,
            parse(&hex[6..8]).map_err(bad_color)?,
        ])),
        _ => Err(bad_color(format!("Unrecognised colour '{}'", value))),
    }
}

fn bad_color(_e: impl std::fmt::Display) -> String {
    "Colour must be a name or #rgb / #rrggbb / #rrggbbaa hex value.".to_string()
}

/// Font lookup order: explicit path → common system fonts.
///
/// v2.0 intentionally ships no TTF (license/size); text overlay therefore needs
/// one of the fonts below, or the caller must pass `fontPath`.
fn resolve_font(explicit: Option<&str>) -> Result<Vec<u8>, String> {
    if let Some(path) = explicit {
        return fs::read(path).map_err(|e| format!("Cannot read font '{}': {}", path, e));
    }

    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Verdana.ttf",
        "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
        "/Library/Fonts/Arial.ttf",
    ];
    #[cfg(target_os = "windows")]
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    #[cfg(target_os = "linux")]
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    const CANDIDATES: &[&str] = &[];

    for candidate in CANDIDATES {
        if let Ok(bytes) = fs::read(candidate) {
            return Ok(bytes);
        }
    }
    Err("No usable TrueType font found. Pass fontPath explicitly (e.g. /System/Library/Fonts/Supplemental/Arial.ttf).".to_string())
}

// ------------------------------------------------------------ vectorization --

#[tauri::command(async)]
pub fn vectorize_image(
    input_path: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let out = match output_path {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => super::output_path(&input, SUFFIX_VECTORIZE, "svg")?,
    };

    #[cfg(feature = "vectorize")]
    {
        let config = vtracer::Config::default();
        vtracer::convert_image_to_svg(&input, &out, config)
            .map_err(|e| format!("Vectorization failed: {}", e))?;
        Ok(format!("Vectorized: {}", out.display()))
    }
    #[cfg(not(feature = "vectorize"))]
    {
        let _ = out;
        Err("Vectorization is not compiled in. Rebuild with `--features vectorize`.".to_string())
    }
}

// ------------------------------------------------------------------- batch --

#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub input: String,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[tauri::command(async)]
pub fn batch_convert<R: Runtime>(
    app: AppHandle<R>,
    inputs: Vec<String>,
    output_format: String,
    quality: Option<u8>,
) -> Result<Vec<BatchResult>, String> {
    let total = inputs.len();
    let mut results = Vec::with_capacity(total);

    for (index, input) in inputs.iter().enumerate() {
        let _ = app.emit(
            "pixconvert://progress",
            ProgressPayload {
                done: index,
                total,
                current: input.clone(),
            },
        );
        let path = PathBuf::from(input);
        let outcome = validate_input(&path).and_then(|_| {
            let format = normalize_format(&output_format);
            let img = decode_any(&app, &path)?;
            let out = output_path(&path, SUFFIX_CONVERT, &format)?;
            encode_any(&app, &img, &out, &format, quality)?;
            Ok(out.display().to_string())
        });
        results.push(batch_entry(input, outcome));
    }

    let _ = app.emit(
        "pixconvert://progress",
        ProgressPayload {
            done: total,
            total,
            current: String::new(),
        },
    );
    Ok(results)
}

#[tauri::command(async)]
pub fn batch_compress<R: Runtime>(
    app: AppHandle<R>,
    inputs: Vec<String>,
    output_format: String,
    quality: u8,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<Vec<BatchResult>, String> {
    let total = inputs.len();
    let mut results = Vec::with_capacity(total);

    for (index, input) in inputs.iter().enumerate() {
        let _ = app.emit(
            "pixconvert://progress",
            ProgressPayload {
                done: index,
                total,
                current: input.clone(),
            },
        );
        let path = PathBuf::from(input);
        let outcome = validate_input(&path).and_then(|_| {
            let format = normalize_format(&output_format);
            let img = decode_any(&app, &path)?;
            let img = maybe_downscale(img, max_width, max_height);
            let out = output_path(&path, SUFFIX_COMPRESS, &format)?;
            encode_any(&app, &img, &out, &format, Some(quality))?;
            Ok(out.display().to_string())
        });
        results.push(batch_entry(input, outcome));
    }

    let _ = app.emit(
        "pixconvert://progress",
        ProgressPayload {
            done: total,
            total,
            current: String::new(),
        },
    );
    Ok(results)
}

fn batch_entry(input: &str, outcome: Result<String, String>) -> BatchResult {
    match outcome {
        Ok(output) => BatchResult {
            input: input.to_string(),
            output: Some(output),
            error: None,
        },
        Err(error) => BatchResult {
            input: input.to_string(),
            output: None,
            error: Some(error),
        },
    }
}

// ------------------------------------------------------------ encode/decode --

/// Decode with the `image` crate, falling back to ffmpeg for HEIC/JXL/PDF.
fn decode_any<R: Runtime>(app: &AppHandle<R>, path: &Path) -> Result<DynamicImage, String> {
    let ext = extension_of(path);
    if ext == "icns" {
        return super::icns::decode(path);
    }
    if matches!(ext.as_str(), "heic" | "heif" | "jxl" | "pdf" | "eps") {
        return decode_via_ffmpeg(app, path);
    }

    // AVIF *decoding* needs the `avif-native` (dav1d) feature; when it is not
    // compiled in, ImageReader returns a clear error and we try ffmpeg instead.
    let result = ImageReader::open(path)
        .map_err(|e| format!("Cannot open {}: {}", path.display(), e))?
        .with_guessed_format()
        .map_err(|e| format!("Cannot inspect {}: {}", path.display(), e))?
        .decode();

    match result {
        Ok(img) => Ok(img),
        Err(native_error) if ext == "avif" => decode_via_ffmpeg(app, path)
            .map_err(|ffmpeg_error| format!("{} (ffmpeg fallback: {})", native_error, ffmpeg_error)),
        Err(e) => Err(format!("Cannot decode {}: {}", path.display(), e)),
    }
}

/// ffmpeg path used for formats the `image` crate cannot handle.
fn decode_via_ffmpeg<R: Runtime>(app: &AppHandle<R>, path: &Path) -> Result<DynamicImage, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = std::env::temp_dir().join(format!(
        "pixconvert-decode-{}-{}.png",
        std::process::id(),
        stamp
    ));
    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        path.display().to_string(),
        temp.display().to_string(),
    ];
    run_tool_named(app, "ffmpeg", &args).map_err(|e| {
        format!(
            "Cannot decode {} (install ffmpeg or convert to PNG first): {}",
            path.display(),
            e
        )
    })?;

    let img = ImageReader::open(&temp)
        .map_err(|e| format!("Cannot open ffmpeg output: {}", e))?
        .decode()
        .map_err(|e| format!("Cannot read ffmpeg output: {}", e));
    let _ = fs::remove_file(&temp);
    img
}

/// Encode to `format`, writing to `out`.
pub(crate) fn encode_any<R: Runtime>(
    app: &AppHandle<R>,
    img: &DynamicImage,
    out: &Path,
    format: &str,
    quality: Option<u8>,
) -> Result<(), String> {
    let quality = quality.unwrap_or(90).clamp(1, 100);

    if matches!(format, "heic" | "heif" | "jxl") {
        return encode_via_ffmpeg(app, img, out, format, quality);
    }

    // ICNS is a container of PNGs, so it is written in one go.
    if format == "icns" {
        let bytes = super::icns::encode(img)?;
        return fs::write(out, bytes).map_err(|e| format!("Cannot write {}: {}", out.display(), e));
    }

    let file = fs::File::create(out).map_err(|e| format!("Cannot create {}: {}", out.display(), e))?;
    let mut writer = BufWriter::new(file);

    let result: Result<(), image::ImageError> = match format {
        "png" => img.write_to(&mut writer, ImageFormat::Png),
        "jpeg" => {
            let rgb = img.to_rgb8();
            JpegEncoder::new_with_quality(&mut writer, quality).encode_image(&rgb)
        }
        // The `image` crate only ships a lossless WebP encoder, so `quality` is
        // ignored here on purpose; the UI explains this.
        "webp" => {
            let rgba = img.to_rgba8();
            WebPEncoder::new_lossless(&mut writer).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
        }
        "avif" => {
            let rgba = img.to_rgba8();
            AvifEncoder::new_with_speed_quality(&mut writer, 6, quality).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
        }
        "gif" => img.write_to(&mut writer, ImageFormat::Gif),
        "bmp" => img.write_to(&mut writer, ImageFormat::Bmp),
        "tiff" => img.write_to(&mut writer, ImageFormat::Tiff),
        "ico" => img.to_rgba8().write_to(&mut writer, ImageFormat::Ico),
        "qoi" => img.write_to(&mut writer, ImageFormat::Qoi),
        other => {
            return Err(format!(
                "Unsupported output format '{}'. Native formats: {}",
                other,
                NATIVE_OUTPUT_FORMATS.join(", ")
            ))
        }
    };

    result.map_err(|e| format!("Cannot write {}: {}", out.display(), e))
}

/// HEIC/JXL/PDF output goes through ffmpeg (documented fallback from §3.7).
fn encode_via_ffmpeg<R: Runtime>(
    app: &AppHandle<R>,
    img: &DynamicImage,
    out: &Path,
    format: &str,
    quality: u8,
) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = std::env::temp_dir().join(format!(
        "pixconvert-encode-{}-{}.png",
        std::process::id(),
        stamp
    ));
    img.save_with_format(&temp, ImageFormat::Png)
        .map_err(|e| format!("Cannot write temporary PNG: {}", e))?;

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        temp.display().to_string(),
    ];
    if format == "jxl" {
        args.extend(["-q:v".to_string(), quality.to_string()]);
    }
    args.push(out.display().to_string());

    let result = run_tool_named(app, "ffmpeg", &args);
    let _ = fs::remove_file(&temp);
    result.map(|_| ()).map_err(|e| {
        format!(
            "Writing {} requires ffmpeg (native {} encoder not compiled in): {}",
            format, format, e
        )
    })
}

fn size_of(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}
