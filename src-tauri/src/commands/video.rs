//! Phase 2 — Video Studio backend (ffmpeg).
//!
//! Every command resolves ffmpeg through [`resolve_tool`] so a portable build in
//! the app data cache works just as well as a system install.

use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, Runtime};

use super::common::run_tool_named;
use super::{extension_of, normalize_format, output_path, validate_input, SUFFIX_COMPRESS, SUFFIX_CONVERT, SUFFIX_EDIT};

const VIDEO_FORMATS: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "wmv", "flv", "m4v", "ts", "gif",
];

const AUDIO_FORMATS: &[&str] = &["mp3", "aac", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];

// --------------------------------------------------------------- conversion --

#[tauri::command(async)]
pub fn convert_video<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    quality: Option<u8>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;
    let format = normalize_format(&output_format);

    if AUDIO_FORMATS.contains(&format.as_str()) {
        return video_to_audio(app, input_path, format, None);
    }
    if format == "gif" {
        return video_to_gif(app, input_path, 15, 480);
    }
    if !VIDEO_FORMATS.contains(&format.as_str()) {
        return Err(format!(
            "Unsupported video format '{}'. Supported: {}",
            format,
            VIDEO_FORMATS.join(", ")
        ));
    }

    let out = output_path(&input, SUFFIX_CONVERT, &format)?;
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
    ];
    args.extend(video_codec_args(&format, quality));
    args.push(out.display().to_string());

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Converted", &out))
}

/// Codec flags per container. GIF/audio targets are handled before this point.
fn video_codec_args(format: &str, quality: Option<u8>) -> Vec<String> {
    let quality = quality.unwrap_or(80).clamp(1, 100);
    let crf = (35.0 - (quality as f32 * 0.17)).round().clamp(16.0, 40.0);
    let crf = format!("{:.0}", crf);

    match format {
        "webm" => vec![
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-crf".into(),
            crf,
            "-b:v".into(),
            "0".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a".into(),
            "96k".into(),
        ],
        "avi" => vec![
            "-c:v".into(),
            "mpeg4".into(),
            "-q:v".into(),
            "5".into(),
            "-c:a".into(),
            "libmp3lame".into(),
        ],
        "wmv" => vec![
            "-c:v".into(),
            "wmv2".into(),
            "-q:v".into(),
            "5".into(),
            "-c:a".into(),
            "wmav2".into(),
        ],
        "mp4" | "m4v" => vec![
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf,
            "-preset".into(),
            "medium".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-movflags".into(),
            "+faststart".into(),
        ],
        _ => vec![
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf,
            "-preset".into(),
            "medium".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
        ],
    }
}

// ------------------------------------------------------------- compression --

#[tauri::command(async)]
pub fn compress_video<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    crf: u8,
    preset: String,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;
    let format = normalize_format(&output_format);
    let crf = crf.clamp(0, 51);
    let preset = if preset.trim().is_empty() {
        "medium".to_string()
    } else {
        preset
    };

    let out = output_path(&input, SUFFIX_COMPRESS, &format)?;
    let mut args = base_args(&input);
    args.extend([
        "-c:v".into(),
        if format == "webm" { "libvpx-vp9" } else { "libx264" }.into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        preset,
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        if format == "webm" { "libopus" } else { "aac" }.into(),
        "-b:a".into(),
        "128k".into(),
    ]);
    args.push(out.display().to_string());

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Compressed", &out))
}

// ------------------------------------------------------------------ editing --

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimOp {
    #[serde(default)]
    pub start: f64,
    /// Seconds; omit to keep everything after `start`.
    #[serde(default)]
    pub end: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCropOp {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoScaleOp {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTextOp {
    pub content: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_text_size")]
    pub size: u32,
    #[serde(default = "default_text_color")]
    pub color: String,
}

fn default_text_size() -> u32 {
    32
}

fn default_text_color() -> String {
    "white".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditOps {
    #[serde(default)]
    pub trim: Option<TrimOp>,
    #[serde(default)]
    pub crop: Option<VideoCropOp>,
    /// 0 / 90 / 180 / 270 clockwise.
    #[serde(default)]
    pub rotate: Option<i32>,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    #[serde(default)]
    pub scale: Option<VideoScaleOp>,
    /// 0.5 = half speed, 2.0 = double speed.
    #[serde(default)]
    pub speed: Option<f32>,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub text: Option<VideoTextOp>,
    /// Optional track mixed in place of the original audio (`-map 1:a`).
    #[serde(default)]
    pub audio_path: Option<String>,
}

#[tauri::command(async)]
pub fn edit_video<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    ops: VideoEditOps,
    output_format: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let format = match output_format {
        Some(fmt) if !fmt.trim().is_empty() => normalize_format(&fmt),
        _ => {
            let ext = extension_of(&input);
            if ext.is_empty() {
                "mp4".to_string()
            } else {
                ext
            }
        }
    };

    let out = output_path(&input, SUFFIX_EDIT, &format)?;
    let mut args = vec!["-y".to_string()];

    if let Some(trim) = &ops.trim {
        if trim.start > 0.0 {
            args.push("-ss".into());
            args.push(format!("{:.3}", trim.start));
        }
    }
    args.extend(base_args(&input));

    if let Some(audio) = &ops.audio_path {
        args.push("-i".to_string());
        args.push(audio.clone());
        args.extend(["-map".into(), "0:v:0".into(), "-map".into(), "1:a:0".into()]);
    } else {
        args.extend([
            "-map".into(),
            "0:v:0?".into(),
            "-map".into(),
            "0:a:0?".into(),
        ]);
    }

    if let Some(trim) = &ops.trim {
        if let (Some(end), true) = (trim.end, trim.end.unwrap_or(0.0) > trim.start) {
            args.push("-t".into());
            args.push(format!("{:.3}", end - trim.start));
        }
    }

    if let Some(vf) = build_video_filter(&ops) {
        args.push("-vf".into());
        args.push(vf);
    }
    if let Some(af) = build_audio_filter(&ops) {
        args.push("-af".into());
        args.push(af);
    }

    if ops.mute {
        args.push("-an".into());
    }
    if ops.audio_path.is_some() {
        args.push("-shortest".into());
    }

    if format == "webm" {
        args.extend(["-c:v".into(), "libvpx-vp9".into(), "-c:a".into(), "libopus".into()]);
    } else if format == "mp4" || format == "mov" || format == "mkv" {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "medium".into(),
            "-c:a".into(),
            "aac".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]);
    } else {
        args.extend(["-c:v".into(), "libx264".into(), "-c:a".into(), "aac".into()]);
    }
    args.push(out.display().to_string());

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Edited", &out))
}

fn build_video_filter(ops: &VideoEditOps) -> Option<String> {
    let mut filters: Vec<String> = Vec::new();

    if let Some(crop) = &ops.crop {
        if crop.width > 0 && crop.height > 0 {
            filters.push(format!(
                "crop={}:{}:{}:{}",
                even(crop.width),
                even(crop.height),
                crop.x,
                crop.y
            ));
        }
    }
    if let Some(scale) = &ops.scale {
        if scale.width > 0 || scale.height > 0 {
            let w = if scale.width > 0 {
                even(scale.width).to_string()
            } else {
                "-2".to_string()
            };
            let h = if scale.height > 0 {
                even(scale.height).to_string()
            } else {
                "-2".to_string()
            };
            filters.push(format!("scale={}:{}", w, h));
        }
    }
    match ops.rotate.unwrap_or(0).rem_euclid(360) {
        90 => filters.push("transpose=1".into()),
        180 => filters.push("hflip,vflip".into()),
        270 => filters.push("transpose=2".into()),
        _ => {}
    }
    if ops.flip_h {
        filters.push("hflip".into());
    }
    if ops.flip_v {
        filters.push("vflip".into());
    }
    if let Some(speed) = ops.speed.filter(|s| *s > 0.0 && (*s - 1.0).abs() > 0.001) {
        filters.push(format!("setpts=PTS/{:.4}", speed));
    }
    if let Some(text) = &ops.text {
        if !text.content.trim().is_empty() {
            filters.push(format!(
                "drawtext=text='{}':x={}:y={}:fontsize={}:fontcolor={}",
                escape_drawtext(&text.content),
                text.x.unwrap_or(16),
                text.y.unwrap_or(16),
                text.size,
                text.color
            ));
        }
    }

    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

fn build_audio_filter(ops: &VideoEditOps) -> Option<String> {
    let mut filters: Vec<String> = Vec::new();
    if let Some(speed) = ops.speed.filter(|s| *s > 0.0 && (*s - 1.0).abs() > 0.001) {
        // atempo accepts 0.5..=100; chain for values outside that range.
        let mut remaining = speed;
        while remaining < 0.5 {
            filters.push("atempo=0.5".into());
            remaining /= 0.5;
        }
        while remaining > 100.0 {
            filters.push("atempo=100".into());
            remaining /= 100.0;
        }
        filters.push(format!("atempo={:.4}", remaining));
    }
    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

fn escape_drawtext(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace(':', "\\:")
        .replace('%', "\\%")
}

fn even(value: u32) -> u32 {
    if value % 2 == 0 {
        value
    } else {
        value - 1
    }
}

// ------------------------------------------------- video -> audio / -> gif --

#[tauri::command(async)]
pub fn video_to_audio<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    audio_format: String,
    bitrate: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;
    let format = normalize_format(&audio_format);

    let out = output_path(&input, SUFFIX_CONVERT, &format)?;
    let mut args = base_args(&input);
    args.extend(["-vn".into()]);
    args.extend(super::audio::audio_codec_args(&format)?);
    if let Some(rate) = bitrate.filter(|b| !b.trim().is_empty()) {
        args.extend(["-b:a".into(), rate]);
    }
    args.push(out.display().to_string());

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Extracted audio", &out))
}

#[tauri::command(async)]
pub fn video_to_gif<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    fps: u32,
    width: u32,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let out = output_path(&input, SUFFIX_CONVERT, "gif")?;
    let fps = fps.clamp(1, 60);
    let mut filters = format!("fps={}", fps);
    if width > 0 {
        filters.push_str(&format!(",scale={}:-1:flags=lanczos", even(width).max(2)));
    }
    filters.push_str(",split[s0][s1];[s0]palettegen=max_colors=256[p];[s1][p]paletteuse=dither=bayer");

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
        "-vf".to_string(),
        filters,
        "-loop".to_string(),
        "0".to_string(),
        out.display().to_string(),
    ];

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Saved GIF", &out))
}

// ---------------------------------------------------------------- helpers ---

fn base_args(input: &std::path::Path) -> Vec<String> {
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
    ]
}

fn done_message(verb: &str, out: &std::path::Path) -> String {
    let size = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    format!("{}: {} ({} bytes)", verb, out.display(), size)
}
