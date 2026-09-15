// src-tauri/src/lib.rs

use std::path::{Path};
use image::ImageReader;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![convert_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn convert_image(input_path: String, output_format: String) -> Result<String, String> {
    let input = Path::new(&input_path);

    // Validate input exists
    if !input.exists() {
        return Err("Input file does not exist".to_string());
    }

    // Load image
    let img = ImageReader::open(input)
        .map_err(|e| format!("Failed to open image: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Generate output path (same directory as input)
    let stem = input.file_stem().unwrap().to_str().unwrap();
    let parent = input.parent().unwrap();
    let output_path = parent.join(format!("{}_converted_by_alris.{}", stem, output_format));

    // Save in requested format
    match output_format.to_lowercase().as_str() {
        "jpg" | "jpeg" => {
            img.save_with_format(&output_path, image::ImageFormat::Jpeg)
                .map_err(|e| format!("Failed to save JPEG: {}", e))?;
        }
        "png" => {
            img.save_with_format(&output_path, image::ImageFormat::Png)
                .map_err(|e| format!("Failed to save PNG: {}", e))?;
        }
        "webp" => {
            img.save_with_format(&output_path, image::ImageFormat::WebP)
                .map_err(|e| format!("Failed to save WebP: {}", e))?;
        }
        "gif" => {
            img.save_with_format(&output_path, image::ImageFormat::Gif)
                .map_err(|e| format!("Failed to save GIF: {}", e))?;
        }
        _ => return Err(format!("Unsupported format: {}", output_format)),
    }

    Ok(format!(
        "Successfully converted to {}",
        output_path.display()
    ))
}
