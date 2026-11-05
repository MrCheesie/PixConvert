//! Shared tool handling: PATH detection, on-demand download and caching.
//!
//! ffmpeg / pandoc are never bundled (see INSTRUCTIONS.md §10). We look for them
//! on `PATH` first, then in the app data cache (`<app_data>/tools`), and only
//! then download a portable build.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::find_on_path;

/// Download sources for the portable builds we know how to unpack.
///
/// ffmpeg comes from the `ffmpeg-static` release assets, which publish both
/// `ffmpeg-<platform>.gz` and `ffprobe-<platform>.gz` (the timeline needs the
/// probe binary too). The previous macOS source (evermeet.cx) was retired
/// upstream and now answers `410 Gone`, so no platform uses it any more.
const FFMPEG_STATIC_RELEASE_API: &str =
    "https://api.github.com/repos/eugeneware/ffmpeg-static/releases/latest";

const PANDOC_RELEASE_API: &str = "https://api.github.com/repos/jgm/pandoc/releases/latest";

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub ffmpeg: Option<String>,
    pub pandoc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadProgress {
    name: String,
    done: u64,
    total: u64,
}

/// Payload for `pixconvert://progress` (batch jobs + long conversions).
#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

/// Payload for `pixconvert://job-progress` (one file being encoded by ffmpeg).
#[derive(Debug, Clone, Serialize)]
pub struct JobProgress {
    pub path: String,
    pub percent: f64,
}

/// Report what is available without triggering any download.
#[tauri::command(async)]
pub fn detect_tools<R: Runtime>(app: AppHandle<R>) -> ToolStatus {
    let locate = |name: &str| -> Option<String> {
        find_on_path(name)
            .or_else(|| cached_tool_path(&app, name).ok().filter(|p| p.is_file()))
            .map(|p| p.display().to_string())
    };
    ToolStatus {
        ffmpeg: locate("ffmpeg"),
        pandoc: locate("pandoc"),
    }
}

/// Download (or reuse) a tool and return its absolute path.
///
/// `async` matters: a synchronous Tauri command runs on the main thread, so the
/// blocking HTTP download below would freeze the window (the macOS "spinning
/// beach ball") for as long as the transfer takes.
#[tauri::command(async)]
pub fn ensure_tool<R: Runtime>(app: AppHandle<R>, name: String) -> Result<String, String> {
    resolve_tool(&app, &name).map(|p| p.display().to_string())
}

/// PATH -> cache -> download. Internal helper shared by every studio command.
pub fn resolve_tool<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<PathBuf, String> {
    if let Some(found) = find_on_path(name) {
        return Ok(found);
    }

    let cached = cached_tool_path(app, name)?;
    if cached.is_file() {
        return Ok(cached);
    }

    download_tool(app, name, &cached)?;
    Ok(cached)
}

fn tools_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?
        .join("tools");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create tools dir: {}", e))?;
    Ok(dir)
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

fn cached_tool_path<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<PathBuf, String> {
    Ok(tools_dir(app)?.join(exe_name(name)))
}

// ---------------------------------------------------------------- downloads --

fn download_tool<R: Runtime>(app: &AppHandle<R>, name: &str, target: &Path) -> Result<(), String> {
    let dir = tools_dir(app)?;
    match name {
        "ffmpeg" => install_ffmpeg(app, &dir),
        "pandoc" => install_pandoc(app, &dir, target),
        other => Err(format!("No download source known for tool '{}'", other)),
    }
}

/// `<platform>` suffix used by the ffmpeg-static release assets.
fn ffmpeg_platform() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            return Some("darwin-arm64");
        }
        if cfg!(target_arch = "x86_64") {
            return Some("darwin-x64");
        }
    }
    #[cfg(target_os = "linux")]
    {
        if cfg!(target_arch = "x86_64") {
            return Some("linux-x64");
        }
        if cfg!(target_arch = "aarch64") {
            return Some("linux-arm64");
        }
        if cfg!(target_arch = "arm") {
            return Some("linux-arm");
        }
        if cfg!(target_arch = "x86") {
            return Some("linux-ia32");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if cfg!(target_arch = "x86_64") {
            return Some("win32-x64");
        }
    }
    None
}

/// Download `ffmpeg` (and its `ffprobe` companion) as gzipped static binaries.
pub fn install_ffmpeg<R: Runtime>(app: &AppHandle<R>, dir: &Path) -> Result<(), String> {
    let platform = ffmpeg_platform().ok_or_else(|| {
        "Automatic ffmpeg download is not available for this platform. Install ffmpeg manually and put it on your PATH."
            .to_string()
    })?;

    emit_progress(app, "ffmpeg", 0, 0);
    for (tool, url) in ffmpeg_static_assets(platform)? {
        let target = dir.join(exe_name(&tool));
        let archive = dir.join(format!("{}-download.gz", tool));

        download_file(app, &tool, &url, &archive).map_err(|e| {
            format!(
                "Could not download {} from {}: {}\nDownload it manually and put it on your PATH.",
                tool, url, e
            )
        })?;
        gunzip(&archive, &target)?;
        let _ = fs::remove_file(&archive);
        make_executable(&target)?;
    }
    Ok(())
}

/// Ask the ffmpeg-static release for the asset URLs of this platform.
fn ffmpeg_static_assets(platform: &str) -> Result<Vec<(String, String)>, String> {
    let body = http_client()?
        .get(FFMPEG_STATIC_RELEASE_API)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Could not query the ffmpeg download list: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Bad ffmpeg release JSON: {}", e))?;
    let assets = json
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "The ffmpeg release JSON had no assets.".to_string())?;

    let mut found: Vec<(String, String)> = Vec::new();
    for tool in ["ffmpeg", "ffprobe"] {
        let wanted = format!("{}-{}.gz", tool, platform);
        let url = assets.iter().find_map(|asset| {
            let name = asset.get("name").and_then(|n| n.as_str())?;
            if name != wanted {
                return None;
            }
            asset
                .get("browser_download_url")
                .and_then(|u| u.as_str())
                .map(str::to_string)
        });
        match url {
            Some(url) => found.push((tool.to_string(), url)),
            // ffmpeg alone still converts; only the timeline needs ffprobe.
            None if tool == "ffprobe" => {}
            None => {
                return Err(format!(
                    "The ffmpeg release has no asset named '{}'. Install ffmpeg manually.",
                    wanted
                ))
            }
        }
    }
    Ok(found)
}

fn gunzip(archive: &Path, target: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| format!("Cannot open {}: {}", archive.display(), e))?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut out = File::create(target).map_err(|e| format!("Cannot write {}: {}", target.display(), e))?;
    std::io::copy(&mut decoder, &mut out).map_err(|e| format!("Cannot unpack {}: {}", archive.display(), e))?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Download pandoc's release archive and move the binary into the cache.
fn install_pandoc<R: Runtime>(app: &AppHandle<R>, dir: &Path, target: &Path) -> Result<(), String> {
    let (url, archive_ext) = pandoc_download_url()?;
    let archive = dir.join(format!("pandoc-download.{}", archive_ext));

    emit_progress(app, "pandoc", 0, 0);
    download_file(app, "pandoc", &url, &archive).map_err(|e| {
        format!(
            "Could not download pandoc from {}: {}\nDownload it manually and put it on your PATH.",
            url, e
        )
    })?;

    // Linux/macOS .tar.* archives are handed to the system tar (bsdtar handles xz).
    if archive_ext.starts_with("tar") {
        let status = Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(dir)
            .status()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !status.success() {
            return Err("Failed to extract the downloaded archive with tar.".to_string());
        }
    } else {
        extract_zip(&archive, dir)?;
    }

    let found = find_binary(dir, "pandoc").ok_or_else(|| {
        "Downloaded archive for pandoc did not contain the binary. Install it manually.".to_string()
    })?;

    if found != target {
        fs::rename(&found, target).or_else(|_| {
            fs::copy(&found, target)
                .map(|_| ())
                .map_err(|e| format!("Cannot move pandoc into the tools dir: {}", e))
        })?;
    }
    make_executable(target)?;
    let _ = fs::remove_file(&archive);
    Ok(())
}

/// Pandoc renames its release assets between versions, so read the release JSON
/// and pick the asset whose name matches this platform instead of guessing.
fn pandoc_download_url() -> Result<(String, String), String> {
    let body = http_client()?
        .get(PANDOC_RELEASE_API)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Could not query the pandoc release list: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Bad pandoc release JSON: {}", e))?;

    let assets = json
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "Pandoc release JSON had no assets.".to_string())?;

    let arch = std::env::consts::ARCH;
    let mut best: Option<(String, String)> = None;
    for asset in assets {
        let name = match asset.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let url = match asset.get("browser_download_url").and_then(|u| u.as_str()) {
            Some(u) => u.to_string(),
            None => continue,
        };
        let lower = name.to_lowercase();

        #[cfg(target_os = "macos")]
        let os_ok = lower.contains("macos");
        #[cfg(target_os = "windows")]
        let os_ok = lower.contains("windows");
        #[cfg(target_os = "linux")]
        let os_ok = lower.contains("linux");
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let os_ok = false;

        if !os_ok || lower.contains("sha") || lower.contains("deb") {
            continue;
        }
        let arch_ok = if arch.starts_with("aarch64") || arch.starts_with("arm") {
            lower.contains("arm64")
        } else {
            lower.contains("amd64") || lower.contains("x86_64")
        };
        let ext = if lower.ends_with(".zip") {
            Some("zip")
        } else if lower.ends_with(".tar.gz") {
            Some("tar.gz")
        } else {
            None
        };
        if let Some(ext) = ext {
            if arch_ok {
                return Ok((url, ext.to_string()));
            }
            best.get_or_insert((url, ext.to_string()));
        }
    }

    best.ok_or_else(|| "No suitable pandoc download found for this platform.".to_string())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    // Without timeouts a stalled connection hangs the request (and the window)
    // forever. 30 min total leaves room for a slow 20 MB download.
    reqwest::blocking::Client::builder()
        .user_agent("PixConvert/0.2.2")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|e| format!("Cannot build HTTP client: {}", e))
}

fn download_file<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let mut response = http_client()?
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(dest).map_err(|e| format!("Cannot create archive: {}", e))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let read = response.read(&mut buf).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buf[..read])
            .map_err(|e| format!("Cannot write archive: {}", e))?;
        done += read as u64;
        emit_progress(app, name, done, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn emit_progress<R: Runtime>(app: &AppHandle<R>, name: &str, done: u64, total: u64) {
    let _ = app.emit(
        "pixconvert://tool-download",
        DownloadProgress {
            name: name.to_string(),
            done,
            total,
        },
    );
}

fn extract_zip(archive: &Path, dir: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| format!("Cannot open archive: {}", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Bad zip archive: {}", e))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Bad zip entry: {}", e))?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dir.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {}", e))?;
        }
        let mut sink = File::create(&out).map_err(|e| format!("Cannot write file: {}", e))?;
        std::io::copy(&mut entry, &mut sink).map_err(|e| format!("Cannot copy file: {}", e))?;
    }
    Ok(())
}

/// Depth-first search for an executable called `name` inside a directory.
fn find_binary(dir: &Path, name: &str) -> Option<PathBuf> {
    let wanted = if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };
    let mut stack = vec![dir.to_path_buf()];
    let mut guard = 0;
    while let Some(current) = stack.pop() {
        guard += 1;
        if guard > 5000 {
            break;
        }
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(wanted.as_str()) {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| format!("Cannot stat {}: {}", path.display(), e))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|e| format!("Cannot chmod: {}", e))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

// ------------------------------------------------------------ process helper --

/// Run a tool and return its stdout, or a readable error with the stderr tail.
pub fn run_tool(program: &Path, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", program.display(), e))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(format!(
        "{} failed ({}): {}",
        program.display(),
        output.status,
        tail.join("\n")
    ))
}

/// Resolve a tool and run it in one step.
pub fn run_tool_named<R: Runtime>(app: &AppHandle<R>, name: &str, args: &[String]) -> Result<String, String> {
    let tool = resolve_tool(app, name)?;
    run_tool(&tool, args)
}

/// `-progress pipe:1` makes ffmpeg stream timestamps on stdout that
/// [`progress_fraction`] turns into a percentage.
pub fn progress_args() -> Vec<String> {
    vec![
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]
}

/// Percent (0–100) carried by one `-progress` line, if it has a timestamp.
pub fn progress_fraction(line: &str, duration_secs: Option<f64>) -> Option<f64> {
    let duration = duration_secs.filter(|d| *d > 0.0)?;
    let line = line.trim();

    if let Some(raw) = line.strip_prefix("out_time_us=") {
        let micros: f64 = raw.trim().parse().ok()?;
        return Some((micros / 1_000_000.0 / duration * 100.0).clamp(0.0, 100.0));
    }
    if let Some(raw) = line.strip_prefix("out_time=") {
        let secs = timestamp_secs(raw.trim())?;
        return Some((secs / duration * 100.0).clamp(0.0, 100.0));
    }
    None
}

/// `00:01:02.500000` -> 62.5. ffmpeg prints `-577014:32:22.99` before the
/// first frame; negative timestamps count as zero.
fn timestamp_secs(value: &str) -> Option<f64> {
    let mut parts = value.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    if hours < 0.0 || minutes < 0.0 || seconds < 0.0 {
        return Some(0.0);
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Resolve a tool and run it while forwarding live encode progress.
pub fn run_tool_named_with_progress<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    args: &[String],
    input: &Path,
    duration_secs: Option<f64>,
) -> Result<String, String> {
    let tool = resolve_tool(app, name)?;
    run_tool_with_progress(app, &tool, args, input, duration_secs)
}

/// Run a long tool, emitting `pixconvert://job-progress` as it works.
///
/// stderr is drained on its own thread: an encode that chats while we only
/// read stdout would otherwise fill the pipe and deadlock.
pub fn run_tool_with_progress<R: Runtime>(
    app: &AppHandle<R>,
    program: &Path,
    args: &[String],
    input: &Path,
    duration_secs: Option<f64>,
) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run {}: {}", program.display(), e))?;

    let stderr_reader = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut text);
            text
        })
    });

    let path = input.display().to_string();
    let mut sent = -1.0f64;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(percent) = progress_fraction(&line, duration_secs) else {
                continue;
            };
            if percent - sent >= 0.5 {
                sent = percent;
                emit_job_progress(app, &path, percent);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for {}: {}", program.display(), e))?;
    let stderr = stderr_reader.and_then(|handle| handle.join().ok()).unwrap_or_default();

    if status.success() {
        emit_job_progress(app, &path, 100.0);
        return Ok(String::new());
    }

    let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(format!(
        "{} failed ({}): {}",
        program.display(),
        status,
        tail.join("\n")
    ))
}

fn emit_job_progress<R: Runtime>(app: &AppHandle<R>, path: &str, percent: f64) {
    let _ = app.emit(
        "pixconvert://job-progress",
        JobProgress {
            path: path.to_string(),
            percent,
        },
    );
}

/// Media duration through ffprobe; `None` (indeterminate progress) when the
/// probe is unavailable, so a missing ffprobe never fails a job.
pub fn probe_duration<R: Runtime>(app: &AppHandle<R>, path: &Path) -> Option<f64> {
    let ffprobe = find_ffprobe(app)?;
    let args = vec![
        "-v".to_string(),
        "error".to_string(),
        "-show_entries".to_string(),
        "format=duration".to_string(),
        "-of".to_string(),
        "default=noprint_wrappers=1:nokey=1".to_string(),
        path.display().to_string(),
    ];
    run_tool(&ffprobe, &args)
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|d| *d > 0.0)
}

// --------------------------------------------------------------- probing ----

/// Basic metadata used by the video/audio timelines.
#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub path: String,
    pub duration_secs: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

/// Locate `ffprobe` (PATH, cache, then next to the resolved ffmpeg).
fn find_ffprobe<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    if let Some(path) = find_on_path("ffprobe") {
        return Some(path);
    }
    if let Ok(cached) = cached_tool_path(app, "ffprobe") {
        if cached.is_file() {
            return Some(cached);
        }
    }
    let ffmpeg = resolve_tool(app, "ffmpeg").ok()?;
    let sibling = ffmpeg.with_file_name(exe_name("ffprobe"));
    sibling.is_file().then_some(sibling)
}

/// Read duration / stream details with ffprobe.
///
/// The frontend uses this to draw the timeline for a single selected file.
#[tauri::command(async)]
pub fn probe_media<R: Runtime>(app: AppHandle<R>, path: String) -> Result<MediaInfo, String> {
    let input = std::path::PathBuf::from(&path);
    super::validate_input(&input)?;

    let ffprobe = find_ffprobe(&app).ok_or_else(|| {
        "ffprobe is required for timelines. Install ffmpeg (it ships ffprobe) or use the Download button."
            .to_string()
    })?;

    let args = vec![
        "-v".to_string(),
        "error".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_format".to_string(),
        "-show_streams".to_string(),
        path.clone(),
    ];
    let raw = run_tool(&ffprobe, &args)?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Bad ffprobe output: {}", e))?;

    let mut info = MediaInfo {
        path,
        duration_secs: json
            .get("format")
            .and_then(|f| f.get("duration"))
            .and_then(|d| d.as_str())
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0),
        has_video: false,
        has_audio: false,
        width: None,
        height: None,
        fps: None,
        sample_rate: None,
        channels: None,
    };

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            let kind = stream
                .get("codec_type")
                .and_then(|k| k.as_str())
                .unwrap_or_default();
            match kind {
                "video" => {
                    info.has_video = true;
                    info.width = stream
                        .get("width")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                    info.height = stream
                        .get("height")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                    info.fps = stream
                        .get("avg_frame_rate")
                        .and_then(|v| v.as_str())
                        .and_then(parse_ratio);
                }
                "audio" => {
                    info.has_audio = true;
                    info.sample_rate = stream
                        .get("sample_rate")
                        .and_then(|v| v.as_str())
                        .and_then(|v| v.parse::<u32>().ok());
                    info.channels = stream
                        .get("channels")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                }
                _ => {}
            }
        }
    }

    if info.duration_secs <= 0.0 {
        // Containers without a format-level duration can still expose one per stream.
        if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
            for stream in streams {
                if let Some(d) = stream
                    .get("duration")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<f64>().ok())
                {
                    info.duration_secs = info.duration_secs.max(d);
                }
            }
        }
    }

    Ok(info)
}

/// "30000/1001" -> 29.97
fn parse_ratio(value: &str) -> Option<f64> {
    let (num, den) = value.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

// ------------------------------------------------------------ text file I/O --

/// Read a UTF-8 text file (used by the doc editor).
#[tauri::command(async)]
pub fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {}", path, e))
}

/// Write a UTF-8 text file. Does not create parent directories.
#[tauri::command(async)]
pub fn write_text_file(path: String, contents: String) -> Result<String, String> {
    fs::write(&path, contents).map_err(|e| format!("Cannot write {}: {}", path, e))?;
    Ok(format!("Saved {}", path))
}

#[cfg(test)]
mod tests {
    use super::progress_fraction;

    fn close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a percentage");
        assert!(
            (actual - expected).abs() < 0.001,
            "expected {}, got {}",
            expected,
            actual
        );
    }

    #[test]
    fn reads_microsecond_timestamps() {
        close(progress_fraction("out_time_us=2500000", Some(10.0)), 25.0);
        close(progress_fraction("out_time_us=0", Some(10.0)), 0.0);
    }

    #[test]
    fn reads_clock_timestamps() {
        close(progress_fraction("out_time=00:00:02.500000", Some(10.0)), 25.0);
        close(progress_fraction("out_time=00:01:00.000000", Some(60.0)), 100.0);
    }

    #[test]
    fn clamps_and_ignores_noise() {
        close(progress_fraction("out_time_us=999999999", Some(10.0)), 100.0);
        assert_eq!(progress_fraction("frame=12", Some(10.0)), None);
        assert_eq!(progress_fraction("out_time_us=1000", None), None);
        assert_eq!(progress_fraction("out_time=1000", Some(0.0)), None);
        close(progress_fraction("out_time=-577014:32:22.99", Some(10.0)), 0.0);
    }
}
