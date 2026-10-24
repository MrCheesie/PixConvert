//! Network test for the on-demand tool download.
//!
//! Ignored by default because it transfers ~40 MB. Run it explicitly:
//!
//! ```bash
//! cd src-tauri && cargo test --test tool_download -- --ignored --nocapture
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use pixconvert_lib::commands::common;
use tauri::test::MockRuntime;
use tauri::AppHandle;

fn app() -> &'static AppHandle<MockRuntime> {
    static APP: std::sync::OnceLock<AppHandle<MockRuntime>> = std::sync::OnceLock::new();
    APP.get_or_init(|| {
        let app: &'static mut tauri::App<MockRuntime> =
            Box::leak(Box::new(tauri::test::mock_app()));
        app.handle().clone()
    })
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixconvert-dl-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
#[ignore = "downloads ~40 MB from GitHub"]
fn downloads_ffmpeg_and_ffprobe_and_they_run() {
    let dir = work_dir("ffmpeg");
    common::install_ffmpeg(app(), &dir).expect("download ffmpeg");

    for tool in ["ffmpeg", "ffprobe"] {
        let path = dir.join(tool);
        assert!(path.is_file(), "{} was not installed", tool);
        let size = fs::metadata(&path).expect("stat binary").len();
        assert!(size > 1_000_000, "{} looks truncated ({} bytes)", tool, size);

        let output = Command::new(&path)
            .arg("-version")
            .output()
            .unwrap_or_else(|e| panic!("cannot run downloaded {}: {}", tool, e));
        assert!(
            output.status.success(),
            "{} -version failed: {}",
            tool,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::remove_dir_all(&dir).ok();
}
