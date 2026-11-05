//! Backend command modules for PixConvert.
//!
//! Shared helpers live here so every studio writes output files the same way:
//! never touch the input, always write `<stem>_<op>_by_alris.<ext>` next to it.

pub mod audio;
pub mod bg;
pub mod common;
pub mod doc;
pub mod icns;
pub mod image;
pub mod open_with;
pub mod video;

use std::path::{Path, PathBuf};

/// Suffixes used for the generated files (see INSTRUCTIONS.md §0.2).
pub const SUFFIX_CONVERT: &str = "converted";
pub const SUFFIX_COMPRESS: &str = "compressed";
pub const SUFFIX_EDIT: &str = "edited";
pub const SUFFIX_VECTORIZE: &str = "vectorized";
/// Background removal (Apple Vision) writes `<stem>_nobg_by_alris.<ext>`.
pub const SUFFIX_REMOVE_BG: &str = "nobg";

/// Build `<parent>/<stem>_<op>_by_alris.<ext>` for an input file.
pub fn output_path(input: &Path, op: &str, ext: &str) -> Result<PathBuf, String> {
    let parent = input
        .parent()
        .ok_or_else(|| format!("Input has no parent directory: {}", input.display()))?;
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Input has no usable file name: {}", input.display()))?;
    let ext = ext.trim_start_matches('.').to_lowercase();
    Ok(parent.join(format!("{}_{}_by_alris.{}", stem, op, ext)))
}

/// Fail early with a readable message when the input is missing / not a file.
pub fn validate_input(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Input file does not exist: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("Input path is not a file: {}", path.display()));
    }
    Ok(())
}

/// Lower-case extension of a path, or an empty string when there is none.
pub fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// Normalise user-facing format aliases (`jpg` -> `jpeg`).
pub fn normalize_format(format: &str) -> String {
    match format.trim().trim_start_matches('.').to_lowercase().as_str() {
        "jpg" | "jpe" => "jpeg".to_string(),
        other => other.to_string(),
    }
}

/// Home directory without pulling in another crate.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Well-known install prefixes that are missing from a GUI-launched `PATH`.
fn install_prefixes() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // `/etc/paths` and `/etc/paths.d/*` are the canonical macOS list; login
        // shells apply them, but an app started by launchd does not.
        dirs.extend(read_path_list(Path::new("/etc/paths")));
        if let Ok(entries) = std::fs::read_dir("/etc/paths.d") {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            for file in files {
                dirs.extend(read_path_list(&file));
            }
        }
        for prefix in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin", "/opt/local/bin", "/opt/local/sbin"] {
            dirs.push(PathBuf::from(prefix));
        }
    }

    #[cfg(target_os = "linux")]
    {
        for prefix in [
            "/usr/local/bin",
            "/usr/local/sbin",
            "/snap/bin",
            "/home/linuxbrew/.linuxbrew/bin",
            "/nix/var/nix/profiles/default/bin",
            "/run/current-system/sw/bin",
        ] {
            dirs.push(PathBuf::from(prefix));
        }
    }

    #[cfg(target_os = "windows")]
    {
        for var in ["LOCALAPPDATA", "PROGRAMDATA"] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                dirs.push(base.join("Microsoft\\WinGet\\Links"));
                dirs.push(base.join("chocolatey\\bin"));
                dirs.push(base.join("Programs\\ffmpeg\\bin"));
            }
        }
        if let Some(home) = home_dir() {
            dirs.push(home.join("scoop\\shims"));
        }
    }

    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("bin"));
        dirs.push(home.join(".cargo/bin"));
    }

    dirs
}

/// Read a newline-separated list of directories (macOS `/etc/paths*` files).
#[cfg(target_os = "macos")]
fn read_path_list(file: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(file)
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Every directory searched for external tools: `PATH` first, then the usual
/// install prefixes, without duplicates.
///
/// A `.app` launched from Finder inherits launchd's bare `PATH`
/// (`/usr/bin:/bin:/usr/sbin:/sbin`), so a Homebrew/MacPorts/Nix install looks
/// missing even though the same binary runs fine from a terminal.
pub fn tool_search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    dirs.extend(install_prefixes());

    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| !dir.as_os_str().is_empty() && seen.insert(dir.clone()));
    dirs
}

/// Look for `name` inside `dirs` (no caching — callers cache if they care).
pub fn find_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };
    for dir in dirs {
        let candidate = dir.join(&exe);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Locate an executable on `PATH` or in a standard install prefix.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    find_in(&tool_search_dirs(), name)
}
