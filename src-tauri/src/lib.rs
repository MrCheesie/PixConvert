// src-tauri/src/lib.rs
//
// Tauri entry point. All conversion logic lives in `commands/*`; this file only
// registers plugins and the IPC handler table.

pub mod commands;

use commands::{audio, common, doc, image, video};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            // image
            image::convert_image,
            image::compress_image,
            image::edit_image,
            image::vectorize_image,
            image::batch_convert,
            image::batch_compress,
            // video
            video::convert_video,
            video::compress_video,
            video::edit_video,
            video::video_to_audio,
            video::video_to_gif,
            // audio
            audio::convert_audio,
            audio::compress_audio,
            audio::edit_audio,
            audio::audio_waveform,
            // doc
            doc::convert_doc,
            doc::doc_to_pdf,
            doc::compress_doc,
            // shared
            common::detect_tools,
            common::ensure_tool,
            common::probe_media,
            common::read_text_file,
            common::write_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
