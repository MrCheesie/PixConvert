//! Apple Icon Image (`.icns`) support.
//!
//! The `icns` crate handles the container (including the legacy RGB + mask
//! element pairs used by pre-10.7 icons), so this module only converts between
//! its `Image` type and `image::DynamicImage`:
//!
//! * [`decode`] picks the largest icon in the family and returns it as RGBA.
//! * [`encode`] renders the standard 16–512 px ladder and writes PNG-based
//!   elements (what every current macOS release expects).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use icns::{IconFamily, PixelFormat};
use image::{imageops::FilterType, DynamicImage, ImageBuffer, RgbaImage};

/// Sizes written for a new icon, largest first. macOS asks for the 512 px
/// artwork and derives the rest, but Finder/Preview show the small ones.
const ICON_SIZES: [u32; 6] = [512, 256, 128, 64, 32, 16];

/// Decode the highest-resolution image inside an `.icns` file.
pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {}", path.display(), e))?;
    let family = IconFamily::read(BufReader::new(file))
        .map_err(|e| format!("Cannot read {} as an ICNS icon: {}", path.display(), e))?;

    let best = family
        .available_icons()
        .into_iter()
        .max_by_key(|icon| icon.pixel_width() as u64 * icon.pixel_height() as u64)
        .ok_or_else(|| format!("{} contains no icons", path.display()))?;

    let icon = family.get_icon_with_type(best).map_err(|e| {
        format!(
            "Cannot decode the {:?} icon in {}: {}",
            best.ostype(),
            path.display(),
            e
        )
    })?;

    let rgba = icon.convert_to(PixelFormat::RGBA);
    let (width, height) = (rgba.width(), rgba.height());
    let buffer: RgbaImage = ImageBuffer::from_raw(width, height, rgba.into_data().into_vec())
        .ok_or_else(|| format!("{} has an unexpected pixel buffer", path.display()))?;

    Ok(DynamicImage::ImageRgba8(buffer))
}

/// Encode `img` as an `.icns` icon family, squaring and resizing as needed.
pub fn encode(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut family = IconFamily::new();

    for size in ICON_SIZES {
        let square = img.resize_exact(size, size, FilterType::Lanczos3).to_rgba8();
        let icon = icns::Image::from_data(PixelFormat::RGBA, size, size, square.into_raw())
            .map_err(|e| format!("Cannot build the {} px icon: {}", size, e))?;
        family
            .add_icon(&icon)
            .map_err(|e| format!("Cannot add the {} px icon: {}", size, e))?;
    }

    let mut bytes = Vec::new();
    family
        .write(&mut bytes)
        .map_err(|e| format!("Cannot write the ICNS container: {}", e))?;
    Ok(bytes)
}
