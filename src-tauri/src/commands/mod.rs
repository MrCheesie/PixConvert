//! Backend command modules for PixConvert 2.0.
//!
//! Shared helpers live here so every studio writes output files the same way:
//! never touch the input, always write `<stem>_<op>_by_alris.<ext>` next to it.

pub mod audio;
pub mod common;
pub mod doc;
pub mod image;
pub mod video;

use std::path::{Path, PathBuf};

/// Suffixes used for the generated files (see INSTRUCTIONS.md §0.2).
pub const SUFFIX_CONVERT: &str = "converted";
pub const SUFFIX_COMPRESS: &str = "compressed";
pub const SUFFIX_EDIT: &str = "edited";
pub const SUFFIX_VECTORIZE: &str = "vectorized";

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

/// Locate an executable on `PATH` (no caching — callers cache if they care).
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };
    for dir in std::env::split_paths(&std::env::var_os("PATH")?) {
        let candidate = dir.join(&exe);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
