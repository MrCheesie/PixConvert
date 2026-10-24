//! Regression tests for tool discovery.
//!
//! A `.app` opened from Finder inherits launchd's bare `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), so `ffmpeg` / `pandoc` installed through
//! Homebrew look missing even though a terminal finds them. These tests pin the
//! fallback search prefixes that fix that.

use std::path::{Path, PathBuf};

use pixconvert_lib::commands::{common, find_in, find_on_path, tool_search_dirs};
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

/// `PATH` is process-global, so the tests that change it take turns.
fn path_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixconvert-path-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn search_dirs_are_unique_and_cover_platform_prefixes() {
    let dirs = tool_search_dirs();
    assert!(!dirs.is_empty(), "expected search directories");

    let unique: std::collections::HashSet<&PathBuf> = dirs.iter().collect();
    assert_eq!(unique.len(), dirs.len(), "duplicate search dirs: {:?}", dirs);

    #[cfg(target_os = "macos")]
    {
        for prefix in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"] {
            assert!(
                dirs.contains(&PathBuf::from(prefix)),
                "missing macOS install prefix {} in {:?}",
                prefix,
                dirs
            );
        }

        // `/etc/paths` is where macOS keeps the list interactive shells use.
        if let Ok(text) = std::fs::read_to_string("/etc/paths") {
            if let Some(entry) = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
            {
                assert!(
                    dirs.iter().any(|dir| dir == Path::new(entry)),
                    "/etc/paths entry {} was not searched",
                    entry
                );
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for prefix in ["/usr/local/bin", "/snap/bin", "/nix/var/nix/profiles/default/bin"] {
            assert!(dirs.contains(&PathBuf::from(prefix)), "missing {} in {:?}", prefix, dirs);
        }
    }
}

#[test]
fn finds_a_binary_inside_a_given_directory() {
    let dir = work_dir("find");
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    std::fs::write(dir.join(exe), b"#!/bin/sh\n").expect("write fake binary");

    let found = find_in(&[dir.clone()], "ffmpeg").expect("fake binary must be found");
    assert_eq!(found, dir.join(exe));
    assert!(find_in(&[dir.clone()], "pandoc").is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn detect_tools_reports_homebrew_installs_with_a_bare_path() {
    // The exact command the frontend calls on boot, with the PATH a Finder
    // launch provides.
    let _guard = path_lock();
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    let status = common::detect_tools(app().clone());
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    if Path::new("/opt/homebrew/bin/ffmpeg").is_file() || Path::new("/usr/local/bin/ffmpeg").is_file() {
        let path = status.ffmpeg.expect("detect_tools must report the installed ffmpeg");
        assert!(Path::new(&path).is_file(), "reported a missing file: {}", path);
    }
    if Path::new("/opt/homebrew/bin/pandoc").is_file() || Path::new("/usr/local/bin/pandoc").is_file() {
        let path = status.pandoc.expect("detect_tools must report the installed pandoc");
        assert!(Path::new(&path).is_file(), "reported a missing file: {}", path);
    }
}

#[test]
fn detects_a_homebrew_tool_when_path_is_bare() {
    // Simulate a Finder launch: only the system defaults on PATH.
    let _guard = path_lock();
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    let ffmpeg = find_on_path("ffmpeg");
    let pandoc = find_on_path("pandoc");
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let homebrew_ffmpeg = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
        .iter()
        .any(|path| Path::new(path).is_file());
    if homebrew_ffmpeg {
        let found = ffmpeg.expect("Homebrew ffmpeg must be found with a bare PATH");
        assert!(found.is_file(), "resolved a non-file: {}", found.display());
    }

    let homebrew_pandoc = ["/opt/homebrew/bin/pandoc", "/usr/local/bin/pandoc"]
        .iter()
        .any(|path| Path::new(path).is_file());
    if homebrew_pandoc {
        let found = pandoc.expect("Homebrew pandoc must be found with a bare PATH");
        assert!(found.is_file(), "resolved a non-file: {}", found.display());
    }
}
