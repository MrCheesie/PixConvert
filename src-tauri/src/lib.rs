// src-tauri/src/lib.rs
//
// Tauri entry point. All conversion logic lives in `commands/*`; this file only
// registers plugins and the IPC handler table, and forwards “Open With” files.

pub mod commands;

use commands::{audio, bg, common, doc, image, open_with, video};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(open_with::PendingOpenFiles::default())
        .invoke_handler(tauri::generate_handler![
            // image
            image::convert_image,
            image::compress_image,
            image::edit_image,
            image::vectorize_image,
            image::batch_convert,
            image::batch_compress,
            // image (macOS Vision)
            bg::remove_background,
            bg::background_removal_available,
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
            // shell integration
            open_with::take_open_files,
        ])
        .setup(|app| {
            // Windows/Linux put “Open With” paths in argv; so does `open -a`.
            // Keep them queued until the frontend asks for them.
            let from_args = open_with::paths_from_args(std::env::args().skip(1));
            let _ = open_with::queue(app.handle(), from_args);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Opened { urls } = event {
            open_with::queue_and_emit(app_handle, open_with::paths_from_urls(&urls));
        }
    });
}
