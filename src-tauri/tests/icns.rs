//! ICNS handling: the backend must read Apple icon families and write them.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use pixconvert_lib::commands::{icns, image as image_cmd};
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

/// Opaque square with a bright corner, so resizing has something to preserve.
fn sample_png(path: &Path, width: u32, height: u32) {
    let mut img = RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let corner = x < width / 4 && y < height / 4;
        *pixel = if corner {
            Rgba([255, 40, 40, 255])
        } else {
            Rgba([20, 60, 200, 255])
        };
    }
    img.save(path).expect("save sample png");
}

/// Same path extraction the other command tests use.
fn path_from(message: &str) -> PathBuf {
    let start = message.find(": ").map(|i| i + 2).unwrap_or(0);
    let rest = &message[start..];
    let end = rest.rfind(" (").unwrap_or(rest.len());
    PathBuf::from(rest[..end].trim())
}

#[test]
fn encode_writes_a_multi_size_family() {
    let dir = work_dir("icns-encode");
    let png = dir.join("logo.png");
    sample_png(&png, 300, 300);

    let img = image::open(&png).expect("open png");
    let bytes = icns::encode(&img).expect("encode icns");
    assert!(bytes.len() > 1000, "icns payload looks empty: {} bytes", bytes.len());

    let icns_path = dir.join("logo.icns");
    fs::write(&icns_path, &bytes).expect("write icns");

    let family =
        ::icns::IconFamily::read(BufReader::new(fs::File::open(&icns_path).unwrap()))
            .expect("read back icns");
    let sizes: Vec<u32> = family
        .available_icons()
        .iter()
        .map(|icon| icon.pixel_width())
        .collect();
    for expected in [16, 32, 64, 128, 256, 512] {
        assert!(sizes.contains(&expected), "missing {} px icon, got {:?}", expected, sizes);
    }

    // Decoding returns the largest member.
    let decoded = icns::decode(&icns_path).expect("decode icns");
    assert_eq!((decoded.width(), decoded.height()), (512, 512));
    assert_eq!(decoded.color(), image::ColorType::Rgba8);
}

#[test]
fn convert_image_round_trips_png_to_icns_and_back() {
    let dir = work_dir("icns-command");
    let png = dir.join("app-icon.png");
    sample_png(&png, 512, 512);

    let to_icns = image_cmd::convert_image(
        app().clone(),
        png.display().to_string(),
        "icns".into(),
        None,
    )
    .expect("png -> icns");
    let icns_path = path_from(&to_icns);
    assert_eq!(icns_path.extension().and_then(|e| e.to_str()), Some("icns"));
    assert!(icns_path.is_file());

    let back = image_cmd::convert_image(
        app().clone(),
        icns_path.display().to_string(),
        "png".into(),
        None,
    )
    .expect("icns -> png");
    let decoded = image::open(path_from(&back)).expect("open round-tripped png");
    assert_eq!((decoded.width(), decoded.height()), (512, 512));

    // Corner colour survives the round-trip (PNG inside ICNS is lossless).
    let rgba = decoded.to_rgba8();
    assert_eq!(rgba.get_pixel(10, 10)[0], 255, "expected the red corner");
}

#[test]
fn decodes_the_icons_shipped_with_the_app() {
    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.icns");
    assert!(icon.is_file(), "missing app icon at {}", icon.display());

    // Output always lands next to the input, so convert a copy in a temp dir
    // instead of littering the source tree.
    let dir = work_dir("icns-shipped");
    let copy = dir.join("icon.icns");
    fs::copy(&icon, &copy).expect("copy app icon");

    let message = image_cmd::convert_image(app().clone(), copy.display().to_string(), "png".into(), None)
        .expect("decode app icon");
    let decoded = image::open(path_from(&message)).expect("open decoded icon");
    assert_eq!(decoded.width(), decoded.height(), "icons must be square");
    assert!(decoded.width() >= 128, "unexpected icon size {}", decoded.width());
}

#[test]
fn rejects_a_file_that_is_not_an_icon() {
    let dir = work_dir("icns-bad");
    let not_icns = dir.join("fake.icns");
    fs::write(&not_icns, b"definitely not an icon family").expect("write fake");

    let error = icns::decode(&not_icns).expect_err("garbage must not decode");
    assert!(error.contains("ICNS"), "unexpected error: {}", error);
}
