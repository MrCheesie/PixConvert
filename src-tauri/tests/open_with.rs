//! “Open With” plumbing: URL/argv parsing and the pending-files queue.

use std::fs;
use std::path::PathBuf;

use pixconvert_lib::commands::open_with::{self, PendingOpenFiles};
use tauri::test::MockRuntime;
use tauri::{AppHandle, Manager, Url};

fn app() -> &'static AppHandle<MockRuntime> {
    static APP: std::sync::OnceLock<AppHandle<MockRuntime>> = std::sync::OnceLock::new();
    APP.get_or_init(|| {
        let app: &'static mut tauri::App<MockRuntime> =
            Box::leak(Box::new(tauri::test::mock_app()));
        app.manage(PendingOpenFiles::default());
        app.handle().clone()
    })
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixconvert-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn file_urls_become_paths() {
    let dir = work_dir("openwith-urls");
    let spaced = dir.join("My Photo.png");
    fs::write(&spaced, b"png").expect("write sample");

    let url = Url::from_file_path(&spaced).expect("file url");
    assert!(url.as_str().contains("My%20Photo.png"), "url should be escaped: {}", url);

    let paths = open_with::paths_from_urls(&[url]);
    assert_eq!(paths, vec![spaced.display().to_string()]);
}

#[test]
fn non_file_urls_and_missing_files_are_ignored() {
    let dir = work_dir("openwith-filter");
    let missing = dir.join("gone.png");

    let urls = vec![
        Url::parse("https://example.com/photo.png").unwrap(),
        Url::from_file_path(&missing).unwrap(),
    ];
    assert!(open_with::paths_from_urls(&urls).is_empty());
}

#[test]
fn argv_keeps_only_existing_files() {
    let dir = work_dir("openwith-args");
    let real = dir.join("clip.mp4");
    fs::write(&real, b"mp4").expect("write sample");

    let args = vec![
        "-psn_0_123456".to_string(),
        "--flag".to_string(),
        real.display().to_string(),
        dir.join("nope.mp4").display().to_string(),
    ];
    assert_eq!(open_with::paths_from_args(args), vec![real.display().to_string()]);
}

#[test]
fn queue_deduplicates_and_drains() {
    let dir = work_dir("openwith-queue");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    fs::write(&a, b"a").expect("write a");
    fs::write(&b, b"b").expect("write b");
    let (a, b) = (a.display().to_string(), b.display().to_string());

    // Start from a clean queue: other tests share this process.
    let _ = open_with::take_open_files(app().clone());

    assert_eq!(open_with::queue(app(), vec![a.clone(), b.clone()]), vec![a.clone(), b.clone()]);
    // A second delivery of the same file is not queued again.
    assert!(open_with::queue(app(), vec![a.clone()]).is_empty());

    assert_eq!(open_with::take_open_files(app().clone()), vec![a, b]);
    assert!(open_with::take_open_files(app().clone()).is_empty());
}
