//! Phase 3 — Audio Studio backend (ffmpeg for conversion, symphonia for peaks).

use std::fs::File;
use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, Runtime};

use super::common::run_tool_named;
use super::{extension_of, normalize_format, output_path, validate_input, SUFFIX_COMPRESS, SUFFIX_CONVERT, SUFFIX_EDIT};

const PEAK_BUCKETS: usize = 2000;

/// Lossy-only audio containers get a sane default bitrate.
fn is_lossy(format: &str) -> bool {
    matches!(format, "mp3" | "aac" | "m4a" | "ogg" | "opus" | "wma")
}

/// ffmpeg encoder flags for each supported audio container.
pub fn audio_codec_args(format: &str) -> Result<Vec<String>, String> {
    let codec = match format {
        "mp3" => "libmp3lame",
        "aac" => "aac",
        "m4a" => "aac",
        "flac" => "flac",
        "wav" => "pcm_s16le",
        "ogg" => "libvorbis",
        "opus" => "libopus",
        "aiff" | "aif" => "pcm_s16be",
        "wma" | "wmv" => "wmav2",
        other => {
            return Err(format!(
                "Unsupported audio format '{}'. Supported: mp3, aac, m4a, flac, wav, ogg, opus, aiff, wma",
                other
            ))
        }
    };
    Ok(vec!["-c:a".into(), codec.to_string()])
}

#[tauri::command(async)]
pub fn convert_audio<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    bitrate: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;
    let format = normalize_format(&output_format);

    let out = output_path(&input, SUFFIX_CONVERT, &format)?;
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
    ];
    args.extend(super::common::progress_args());
    args.extend(audio_codec_args(&format)?);
    if let Some(rate) = bitrate.filter(|b| !b.trim().is_empty()) {
        args.extend(["-b:a".into(), rate]);
    }
    args.push(out.display().to_string());

    let duration = super::common::probe_duration(&app, &input);
    super::common::run_tool_named_with_progress(&app, "ffmpeg", &args, &input, duration)?;
    Ok(done_message("Converted", &out))
}

#[tauri::command(async)]
pub fn compress_audio<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
    bitrate: String,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;
    let format = normalize_format(&output_format);
    let bitrate = if bitrate.trim().is_empty() {
        "128k".to_string()
    } else {
        bitrate
    };

    let out = output_path(&input, SUFFIX_COMPRESS, &format)?;
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input.display().to_string(),
    ];
    args.extend(super::common::progress_args());
    args.extend(audio_codec_args(&format)?);
    if is_lossy(&format) {
        args.extend(["-b:a".into(), bitrate]);
    }
    args.push(out.display().to_string());

    let duration = super::common::probe_duration(&app, &input);
    super::common::run_tool_named_with_progress(&app, "ffmpeg", &args, &input, duration)?;
    Ok(done_message("Compressed", &out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrimOp {
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEditOps {
    #[serde(default)]
    pub trim: Option<AudioTrimOp>,
    /// 0.5 = half speed, 2.0 = double speed.
    #[serde(default)]
    pub speed: Option<f32>,
    /// Gain in decibels.
    #[serde(default)]
    pub volume_db: Option<f32>,
    #[serde(default)]
    pub fade_in: Option<f32>,
    #[serde(default)]
    pub fade_out: Option<f32>,
    #[serde(default)]
    pub reverse: bool,
    /// Second track mixed under the first one.
    #[serde(default)]
    pub mix_path: Option<String>,
}

#[tauri::command(async)]
pub fn edit_audio<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    ops: AudioEditOps,
    output_format: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let format = match output_format {
        Some(fmt) if !fmt.trim().is_empty() => normalize_format(&fmt),
        _ => {
            let ext = extension_of(&input);
            if ext.is_empty() {
                "mp3".to_string()
            } else {
                ext
            }
        }
    };

    let out = output_path(&input, SUFFIX_EDIT, &format)?;
    let mut args = vec!["-y".to_string()];

    if let Some(trim) = &ops.trim {
        if trim.start > 0.0 {
            args.extend(["-ss".into(), format!("{:.3}", trim.start)]);
        }
    }
    args.extend([
        "-i".to_string(),
        input.display().to_string(),
    ]);

    if let Some(mix) = &ops.mix_path {
        args.extend(["-i".to_string(), mix.clone()]);
        args.extend([
            "-filter_complex".to_string(),
            format!(
                "[0:a]{}[a0];[1:a]volume=0.6[a1];[a0][a1]amix=inputs=2:duration=first:dropout_transition=2[out]",
                audio_filter_chain(&ops)
            ),
            "-map".to_string(),
            "[out]".to_string(),
        ]);
    } else {
        let chain = audio_filter_chain(&ops);
        if !chain.is_empty() {
            args.extend(["-af".into(), chain]);
        }
    }

    if let Some(trim) = &ops.trim {
        if let Some(end) = trim.end.filter(|e| *e > trim.start) {
            args.extend(["-t".into(), format!("{:.3}", end - trim.start)]);
        }
    }

    args.extend(audio_codec_args(&format)?);
    if is_lossy(&format) {
        args.extend(["-b:a".into(), "192k".into()]);
    }
    args.push(out.display().to_string());

    run_tool_named(&app, "ffmpeg", &args)?;
    Ok(done_message("Edited", &out))
}

fn audio_filter_chain(ops: &AudioEditOps) -> String {
    let mut filters: Vec<String> = Vec::new();

    if ops.reverse {
        filters.push("areverse".into());
    }
    if let Some(speed) = ops.speed.filter(|s| *s > 0.0 && (*s - 1.0).abs() > 0.001) {
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
    if let Some(gain) = ops.volume_db.filter(|g| g.abs() > 0.001) {
        filters.push(format!("volume={:.2}dB", gain));
    }
    if let Some(fade) = ops.fade_in.filter(|f| *f > 0.0) {
        filters.push(format!("afade=t=in:st=0:d={:.2}", fade));
    }
    if let Some(fade) = ops.fade_out.filter(|f| *f > 0.0) {
        // `afade` has no "fade from the end" mode, so fade the reversed stream
        // in and reverse it back again.
        filters.push("areverse".into());
        filters.push(format!("afade=t=in:st=0:d={:.2}", fade));
        filters.push("areverse".into());
    }

    filters.join(",")
}

// ---------------------------------------------------------------- waveform --

/// Decode the whole file and reduce it to [`PEAK_BUCKETS`] peak amplitudes.
///
/// Returns values in `0.0..=1.0` — the frontend draws them as a bar chart.
#[tauri::command(async)]
pub fn audio_waveform(input_path: String) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path = PathBuf::from(&input_path);
    validate_input(&path)?;

    let file = File::open(&path).map_err(|e| format!("Cannot open audio file: {}", e))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported audio format: {}", e))?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| "No audio track found in this file.".to_string())?;
    let track_id = track.id;
    let total_frames = track.codec_params.n_frames;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("No decoder for this audio format: {}", e))?;

    let mut peaks: Vec<f32> = Vec::with_capacity(PEAK_BUCKETS);
    let mut sample_index: u64 = 0;
    // Fall back to a 5 minute estimate when the container has no frame count.
    let estimate = total_frames.unwrap_or(sample_rate as u64 * 300);

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("Failed while decoding audio: {}", e)),
        };
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if decoded.frames() == 0 {
                    continue;
                }
                let spec = *decoded.spec();
                let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buffer.copy_interleaved_ref(decoded);
                let channels = spec.channels.count().max(1);

                for frame in buffer.samples().chunks(channels) {
                    let amplitude = frame.iter().fold(0f32, |acc, s| acc.max(s.abs()));
                    let bucket = ((sample_index * PEAK_BUCKETS as u64) / estimate.max(1)) as usize;
                    if bucket < peaks.len() {
                        peaks[bucket] = peaks[bucket].max(amplitude);
                    } else {
                        while peaks.len() <= bucket && peaks.len() < PEAK_BUCKETS * 4 {
                            peaks.push(0.0);
                        }
                        if bucket < peaks.len() {
                            peaks[bucket] = amplitude;
                        }
                    }
                    sample_index += 1;
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(e) => return Err(format!("Failed while decoding audio: {}", e)),
        }
    }

    Ok(decimate_peaks(peaks))
}

/// Squeeze arbitrary bucket counts down to [`PEAK_BUCKETS`] max-pooled values.
fn decimate_peaks(peaks: Vec<f32>) -> Vec<f32> {
    if peaks.len() <= PEAK_BUCKETS {
        return peaks.into_iter().map(normalise).collect();
    }
    let chunk = peaks.len().div_ceil(PEAK_BUCKETS);
    let mut out = Vec::with_capacity(PEAK_BUCKETS);
    for group in peaks.chunks(chunk) {
        let peak = group.iter().fold(0f32, |acc, p| acc.max(*p));
        out.push(normalise(peak));
    }
    out
}

fn normalise(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn done_message(verb: &str, out: &std::path::Path) -> String {
    let size = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    format!("{}: {} ({} bytes)", verb, out.display(), size)
}
