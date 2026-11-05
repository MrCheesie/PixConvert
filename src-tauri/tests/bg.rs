//! Background removal (Apple Vision). The segmentation model only exists on
//! macOS 14+, so these tests check the contract: no panics, same frame size,
//! real transparency, and a clear message when there is nothing to cut out.

use std::fs;
use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use pixconvert_lib::commands::bg;
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
    let dir = std::env::temp_dir().join(format!("pixconvert-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn path_from(message: &str) -> PathBuf {
    let start = message.find(": ").map(|i| i + 2).unwrap_or(0);
    let rest = &message[start..];
    let end = rest.rfind(" (").unwrap_or(rest.len());
    PathBuf::from(rest[..end].trim())
}

/// A bold shape on a plain background — enough for Vision to find a subject.
fn sample_png(path: &std::path::Path) {
    let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as i64 - 200;
        let dy = y as i64 - 150;
        if dx * dx + dy * dy < 90 * 90 {
            *pixel = Rgba([200, 30, 40, 255]);
        }
    }
    img.save(path).expect("save sample png");
}

#[test]
fn missing_input_is_reported() {
    let error = bg::remove_background(app().clone(), "/definitely/not/here.png".into(), None)
        .expect_err("missing input must fail");
    assert!(error.contains("does not exist"), "unexpected error: {}", error);
}

#[test]
fn availability_matches_the_platform() {
    let available = bg::background_removal_available();
    if !cfg!(target_os = "macos") {
        assert!(!available, "only macOS can run the Vision request");
    }
}

#[test]
fn segmentation_produces_a_transparent_uncropped_png() {
    if !bg::background_removal_available() {
        eprintln!("skipping: Apple Vision foreground mask is not available here");
        return;
    }

    let dir = work_dir("bg-shape");
    let png = dir.join("shape.png");
    sample_png(&png);

    match bg::remove_background(app().clone(), png.display().to_string(), Some("png".into())) {
        Ok(message) => {
            let out = path_from(&message);
            assert!(out.is_file(), "output missing: {}", out.display());

            let rgba = image::open(&out).expect("open output").to_rgba8();
            // `croppedToInstancesExtent: false` keeps the original frame.
            assert_eq!((rgba.width(), rgba.height()), (400, 300));

            let total = (rgba.width() * rgba.height()) as usize;
            let transparent = rgba.pixels().filter(|p| p[3] < 10).count();
            let opaque = rgba.pixels().filter(|p| p[3] >= 250).count();
            assert!(
                transparent > total / 4,
                "expected a transparent background, got {} of {} px",
                transparent,
                total
            );
            assert!(
                opaque > total / 20,
                "expected the subject to stay opaque, got {} of {} px",
                opaque,
                total
            );

            // The disc is red in the source; colour must survive the copy.
            let centre = rgba.get_pixel(200, 150);
            assert!(
                centre[0] > 150 && centre[1] < 90 && centre[2] < 90,
                "subject colour changed: {:?}",
                centre
            );
        }
        // A synthetic disc may not register as a subject; that is a valid answer.
        Err(error) => assert!(
            error.contains("No clear subject"),
            "unexpected failure: {}",
            error
        ),
    }
}

/// Manual end-to-end check against a real photo:
/// `PIXCONVERT_BG_PHOTO=/path/photo.jpg cargo test --test bg -- --ignored`
#[test]
#[ignore = "needs PIXCONVERT_BG_PHOTO pointing at a real photo"]
fn removes_the_background_from_a_real_photo() {
    let photo = std::env::var("PIXCONVERT_BG_PHOTO").expect("set PIXCONVERT_BG_PHOTO");

    let message = bg::remove_background(app().clone(), photo.clone(), Some("png".into()))
        .expect("segmentation");
    let out = path_from(&message);
    let rgba = image::open(&out).expect("open cut-out").to_rgba8();

    let total = (rgba.width() * rgba.height()) as usize;
    let transparent = rgba.pixels().filter(|p| p[3] < 10).count();
    let opaque = rgba.pixels().filter(|p| p[3] >= 250).count();
    eprintln!(
        "{}: {}x{}, {:.1}% transparent, {:.1}% opaque -> {}",
        photo,
        rgba.width(),
        rgba.height(),
        transparent as f64 * 100.0 / total as f64,
        opaque as f64 * 100.0 / total as f64,
        out.display()
    );

    assert!(transparent > total / 20, "background was not removed");
    assert!(opaque > total / 20, "subject disappeared");
}
