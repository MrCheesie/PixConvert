//! End-to-end checks for the Tauri commands, using a headless mock runtime.
//!
//! Run with `cd src-tauri && cargo test`. Video/doc tests skip themselves when
//! ffmpeg / pandoc are not installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use image::{Rgb, RgbImage};
use pixconvert_lib::commands::{audio, common, doc, image as image_cmd, video};
use tauri::test::MockRuntime;
use tauri::AppHandle;

fn app() -> &'static AppHandle<MockRuntime> {
    static APP: OnceLock<AppHandle<MockRuntime>> = OnceLock::new();
    APP.get_or_init(|| {
        let app: &'static mut tauri::App<MockRuntime> =
            Box::leak(Box::new(tauri::test::mock_app()));
        app.handle().clone()
    })
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixconvert-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Gradient test image so compression/quality differences are visible.
fn sample_png(path: &Path, width: u32, height: u32) {
    let mut img = RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgb([
            (x * 255 / width.max(1)) as u8,
            (y * 255 / height.max(1)) as u8,
            ((x + y) % 256) as u8,
        ]);
    }
    img.save(path).expect("save sample png");
}

fn has_tool(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Every command replies `"<verb>: <path> (<details>)"`; extract the path.
fn path_from(message: &str) -> PathBuf {
    let start = message.find(": ").map(|i| i + 2).unwrap_or(0);
    let rest = &message[start..];
    let end = rest.rfind(" (").unwrap_or(rest.len());
    PathBuf::from(rest[..end].trim())
}

#[test]
fn converts_to_every_native_format() {
    let dir = work_dir("convert");
    let input = dir.join("sample.png");
    sample_png(&input, 64, 48);

    for format in ["jpeg", "webp", "gif", "bmp", "tiff", "avif", "ico", "qoi"] {
        let message = image_cmd::convert_image(app().clone(), input.display().to_string(), format.into(), Some(80))
            .unwrap_or_else(|e| panic!("convert to {} failed: {}", format, e));
        let out = path_from(&message);
        assert!(out.is_file(), "{} output missing: {}", format, out.display());
        assert!(fs::metadata(&out).unwrap().len() > 0, "{} output is empty", format);
    }

    // png round-trip keeps working too
    let png_message = image_cmd::convert_image(app().clone(), input.display().to_string(), "png".into(), None)
        .expect("png conversion");
    assert!(path_from(&png_message).is_file());
}

#[test]
fn jpeg_quality_changes_file_size() {
    let dir = work_dir("quality");
    let input = dir.join("sample.png");
    sample_png(&input, 320, 240);

    let low = image_cmd::convert_image(app().clone(), input.display().to_string(), "jpeg".into(), Some(5)).expect("low quality");
    let low_size = fs::metadata(path_from(&low)).unwrap().len();
    let high = image_cmd::convert_image(app().clone(), input.display().to_string(), "jpeg".into(), Some(95)).expect("high quality");
    let high_size = fs::metadata(path_from(&high)).unwrap().len();

    assert!(high_size > low_size, "high quality ({} B) should be larger than low ({} B)", high_size, low_size);
}

#[test]
fn compress_respects_max_width_and_keeps_aspect() {
    let dir = work_dir("compress");
    let input = dir.join("sample.png");
    sample_png(&input, 400, 200);

    let message = image_cmd::compress_image(
        app().clone(),
        input.display().to_string(),
        "jpeg".into(),
        40,
        Some(100),
        None,
    )
    .expect("compress");
    let out = path_from(&message);
    let decoded = image::open(&out).expect("open compressed");
    assert_eq!(decoded.width(), 100);
    assert_eq!(decoded.height(), 50);
}

#[test]
fn edit_crop_rotate_and_grayscale() {
    let dir = work_dir("edit");
    let input = dir.join("sample.png");
    sample_png(&input, 64, 48);

    let crop = image_cmd::edit_image(
        app().clone(),
        input.display().to_string(),
        image_cmd::ImageEditOps {
            crop: Some(image_cmd::CropOp { x: 10, y: 5, width: 20, height: 15 }),
            ..Default::default()
        },
        Some("png".into()),
    )
    .expect("crop");
    let cropped = image::open(path_from(&crop)).expect("open crop");
    assert_eq!((cropped.width(), cropped.height()), (20, 15));

    let rotate = image_cmd::edit_image(
        app().clone(),
        input.display().to_string(),
        image_cmd::ImageEditOps { rotate: Some(90), ..Default::default() },
        Some("png".into()),
    )
    .expect("rotate");
    let rotated = image::open(path_from(&rotate)).expect("open rotate");
    assert_eq!((rotated.width(), rotated.height()), (48, 64));

    let gray = image_cmd::edit_image(
        app().clone(),
        input.display().to_string(),
        image_cmd::ImageEditOps { grayscale: true, ..Default::default() },
        Some("png".into()),
    )
    .expect("grayscale");
    let gray_img = image::open(path_from(&gray)).expect("open grayscale").to_rgb8();
    for pixel in gray_img.pixels().take(50) {
        assert_eq!(pixel[0], pixel[1], "grayscale channels must match");
        assert_eq!(pixel[1], pixel[2], "grayscale channels must match");
    }
}

#[test]
fn edit_text_overlays_pixels() {
    let dir = work_dir("text");
    let input = dir.join("blank.png");
    RgbImage::from_pixel(240, 120, Rgb([255, 255, 255])).save(&input).expect("blank");

    let message = image_cmd::edit_image(
        app().clone(),
        input.display().to_string(),
        image_cmd::ImageEditOps {
            text: Some(image_cmd::TextOp {
                content: "Hi".into(),
                x: 10,
                y: 10,
                size: 60.0,
                color: "#ff0000".into(),
                font_path: None,
            }),
            ..Default::default()
        },
        Some("png".into()),
    )
    .expect("text overlay");

    let img = image::open(path_from(&message)).expect("open text").to_rgb8();
    let red_pixels = img
        .pixels()
        .filter(|p| p[0] > 180 && p[1] < 120 && p[2] < 120)
        .count();
    assert!(red_pixels > 20, "expected red text pixels, found {}", red_pixels);
}

#[test]
fn batch_convert_reports_every_file() {
    let dir = work_dir("batch");
    let mut inputs = Vec::new();
    for index in 0..3 {
        let path = dir.join(format!("img{}.png", index));
        sample_png(&path, 32, 32);
        inputs.push(path.display().to_string());
    }

    let results = image_cmd::batch_convert(app().clone(), inputs.clone(), "jpeg".into(), Some(70)).expect("batch");
    assert_eq!(results.len(), 3);
    for result in &results {
        assert!(result.error.is_none(), "batch error: {:?}", result.error);
        assert!(Path::new(result.output.as_ref().unwrap()).is_file());
    }
}

#[test]
fn missing_input_returns_error_instead_of_panicking() {
    let error = image_cmd::convert_image(app().clone(), "/definitely/not/here.png".into(), "png".into(), None)
        .expect_err("missing input must fail");
    assert!(error.contains("does not exist"), "unexpected error: {}", error);
}

#[test]
fn waveform_returns_normalised_peaks() {
    let dir = work_dir("waveform");
    let wav = dir.join("tone.wav");
    write_sine_wav(&wav, 8000, 0.5);

    let peaks = audio::audio_waveform(wav.display().to_string()).expect("waveform");
    assert!(!peaks.is_empty(), "expected peaks");
    assert!(peaks.iter().all(|p| (0.0..=1.0).contains(p)), "peaks must be normalised");
    assert!(peaks.iter().any(|p| *p > 0.2), "expected audible peaks");
}

fn write_sine_wav(path: &Path, sample_rate: u32, seconds: f32) {
    let frames = (sample_rate as f32 * seconds) as u32;
    let mut data = Vec::with_capacity(frames as usize * 2);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.8;
        data.extend_from_slice(&((sample * i16::MAX as f32) as i16).to_le_bytes());
    }

    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    fs::write(path, wav).expect("write wav");
}

#[cfg(feature = "vectorize")]
#[test]
fn vectorize_writes_an_svg() {
    let dir = work_dir("vectorize");
    let input = dir.join("logo.png");
    sample_png(&input, 48, 48);

    let message = image_cmd::vectorize_image(input.display().to_string(), None).expect("vectorize");
    let out = path_from(&message);
    assert!(out.is_file(), "svg missing: {}", out.display());
    let contents = fs::read_to_string(&out).expect("read svg");
    assert!(contents.contains("<svg"), "expected SVG markup");
}

#[test]
fn probe_media_reports_audio_duration_and_streams() {
    if !has_tool("ffprobe") {
        eprintln!("skipping: ffprobe not installed");
        return;
    }
    let dir = work_dir("probe-audio");
    let wav = dir.join("tone.wav");
    write_sine_wav(&wav, 8000, 0.5);

    let info = common::probe_media(app().clone(), wav.display().to_string()).expect("probe audio");
    assert!(info.has_audio, "expected an audio stream");
    assert!(!info.has_video, "wav must not report video");
    assert!((info.duration_secs - 0.5).abs() < 0.05, "duration was {}", info.duration_secs);
    assert_eq!(info.sample_rate, Some(8000));
    assert_eq!(info.channels, Some(1));
}

#[test]
fn probe_media_rejects_missing_files() {
    let error = common::probe_media(app().clone(), "/definitely/not/here.mp4".into())
        .expect_err("missing input must fail");
    assert!(error.contains("does not exist"), "unexpected error: {}", error);
}

#[test]
fn video_pipeline_convert_compress_edit_extract_gif() {
    if !has_tool("ffmpeg") {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = work_dir("video");
    let clip = dir.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi", "-i", "testsrc=duration=2:size=320x240:rate=15",
            "-f", "lavfi", "-i", "sine=frequency=440:duration=2",
            "-shortest",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
        ])
        .arg(&clip)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "could not generate test clip");

    let mkv = video::convert_video(app().clone(), clip.display().to_string(), "mkv".into(), Some(70)).expect("mp4->mkv");
    assert!(path_from(&mkv).is_file());

    let webm = video::convert_video(app().clone(), clip.display().to_string(), "webm".into(), Some(70)).expect("mp4->webm");
    assert!(path_from(&webm).is_file());

    let compressed = video::compress_video(app().clone(), clip.display().to_string(), "mp4".into(), 35, "veryfast".into(), None)
        .expect("compress video");
    assert!(path_from(&compressed).is_file());

    // Low CPU mode must still produce a playable file, just with one thread.
    let low_cpu = video::compress_video(app().clone(), clip.display().to_string(), "mp4".into(), 35, "veryfast".into(), Some(true))
        .expect("compress video in low CPU mode");
    assert!(path_from(&low_cpu).is_file());

    let audio_out = video::video_to_audio(app().clone(), clip.display().to_string(), "mp3".into(), Some("96k".into()))
        .expect("extract audio");
    assert!(path_from(&audio_out).is_file());

    let info = common::probe_media(app().clone(), clip.display().to_string()).expect("probe video");
    assert!(info.has_video && info.has_audio, "clip should have both streams");
    assert_eq!((info.width, info.height), (Some(320), Some(240)));
    assert!((info.duration_secs - 2.0).abs() < 0.25, "duration was {}", info.duration_secs);

    let gif = video::video_to_gif(app().clone(), clip.display().to_string(), 10, 160).expect("gif");
    assert!(path_from(&gif).is_file());

    let edited = video::edit_video(
        app().clone(),
        clip.display().to_string(),
        video::VideoEditOps {
            trim: Some(video::TrimOp { start: 0.5, end: Some(1.5) }),
            speed: Some(1.0),
            ..Default::default()
        },
        Some("mp4".into()),
    )
    .expect("trim video");
    assert!(path_from(&edited).is_file());
}

#[test]
fn audio_convert_compress_and_edit() {
    if !has_tool("ffmpeg") {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = work_dir("audio");
    let wav = dir.join("tone.wav");
    write_sine_wav(&wav, 44100, 1.0);

    let mp3 = audio::convert_audio(app().clone(), wav.display().to_string(), "mp3".into(), Some("128k".into()))
        .expect("wav->mp3");
    assert!(path_from(&mp3).is_file());

    let flac = audio::convert_audio(app().clone(), wav.display().to_string(), "flac".into(), None).expect("wav->flac");
    assert!(path_from(&flac).is_file());

    let compressed = audio::compress_audio(app().clone(), wav.display().to_string(), "mp3".into(), "64k".into())
        .expect("compress audio");
    assert!(path_from(&compressed).is_file());

    let edited = audio::edit_audio(
        app().clone(),
        wav.display().to_string(),
        audio::AudioEditOps {
            trim: Some(audio::AudioTrimOp { start: 0.2, end: Some(0.7) }),
            volume_db: Some(-3.0),
            ..Default::default()
        },
        Some("wav".into()),
    )
    .expect("edit audio");
    let out = path_from(&edited);
    assert!(out.is_file());
    assert!(fs::metadata(&out).unwrap().len() < fs::metadata(&wav).unwrap().len());
}

#[test]
fn doc_convert_markdown_to_html_and_pdf() {
    if !has_tool("pandoc") {
        eprintln!("skipping: pandoc not installed");
        return;
    }
    let dir = work_dir("doc");
    let md = dir.join("note.md");
    fs::write(&md, "# Title\n\nHello **world**.\n").expect("write md");

    let html = doc::convert_doc(app().clone(), md.display().to_string(), "html".into()).expect("md->html");
    let html_path = path_from(&html);
    let contents = fs::read_to_string(&html_path).expect("read html");
    assert!(contents.to_lowercase().contains("<h1"), "expected a heading in {}", html_path.display());

    // PDF needs a TeX engine or typst; only assert when one exists.
    if has_tool("pdflatex") || has_tool("typst") {
        let pdf = doc::doc_to_pdf(app().clone(), md.display().to_string()).expect("md->pdf");
        assert!(path_from(&pdf).is_file());
    }
}
