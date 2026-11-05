//! “Open With” support: files handed to the app by Finder / Explorer / the
//! shell, or by `open -a PixConvert <files>`.
//!
//! macOS delivers them through [`tauri::RunEvent::Opened`]; Windows and Linux
//! put them in `argv`. Both paths end up in [`PendingOpenFiles`], and the
//! frontend drains that queue once it is mounted (the macOS event can fire
//! before the webview has a listener).

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, Url};

/// Event carrying `string[]` of absolute paths to the frontend.
pub const OPEN_FILES_EVENT: &str = "pixconvert://open-files";

/// Paths that arrived before the UI was listening (or while it was busy).
#[derive(Default)]
pub struct PendingOpenFiles(Mutex<Vec<String>>);

fn existing_file(path: PathBuf) -> Option<String> {
    if path.is_file() {
        Some(path.display().to_string())
    } else {
        None
    }
}

/// `file://` URLs from `RunEvent::Opened`, percent-decoded, files only.
pub fn paths_from_urls(urls: &[Url]) -> Vec<String> {
    urls.iter()
        .filter(|url| url.scheme() == "file")
        .filter_map(|url| url.to_file_path().ok())
        .filter_map(existing_file)
        .collect()
}

/// Paths passed as command-line arguments (Windows Explorer, `open -a`, a
/// terminal). Flags such as macOS' `-psn_0_1234` are ignored.
pub fn paths_from_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .filter_map(existing_file)
        .collect()
}

/// Queue `paths`, skipping anything already waiting, and return the new ones.
pub fn queue<R: Runtime>(app: &AppHandle<R>, paths: Vec<String>) -> Vec<String> {
    let pending = app.state::<PendingOpenFiles>();
    // A poisoned lock has no data worth keeping; recovering beats panicking in
    // a `setup` callback.
    let mut queued = pending
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut added = Vec::new();
    for path in paths {
        if !queued.contains(&path) {
            queued.push(path.clone());
            added.push(path);
        }
    }
    added
}

/// Queue the paths and tell a listening frontend about them.
pub fn queue_and_emit<R: Runtime>(app: &AppHandle<R>, paths: Vec<String>) {
    let added = queue(app, paths);
    if !added.is_empty() {
        let _ = app.emit(OPEN_FILES_EVENT, added);
    }
}

/// Drain the queue. Called by the frontend right after it starts listening.
#[tauri::command(async)]
pub fn take_open_files<R: Runtime>(app: AppHandle<R>) -> Vec<String> {
    let pending = app.state::<PendingOpenFiles>();
    let mut queued = pending
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut *queued)
}
