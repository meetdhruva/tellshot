//! Golden-image tests: rendering must not change unless we mean it to.
//!
//! After an intended visual change, run `cargo xtask bless` and review the new images in the diff.
//! On failure the actual output is written next to the golden file as `*.actual.png`.
#![expect(clippy::unwrap_used, clippy::panic, reason = "test helpers: failing loudly is the point")]

use std::path::PathBuf;

use tellshot_core::{BaseImage, Document, Edit, History, Point, Rect, Shape, Style, palette};
use tellshot_format::png_codec::{self, Effort};
use tellshot_imaging::Image;
use tellshot_render::{Fonts, render};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden")
}

/// A deterministic "screenshot": a soft gradient with dark text-like bars.
fn base(width: u32, height: u32) -> Image {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let bar = y % 24 < 6 && x % 90 < 70 && x > 10;
            let (r, g, b) = if bar {
                (40, 44, 52)
            } else {
                (
                    u8::try_from(200 + x * 40 / width).unwrap(),
                    u8::try_from(210 + y * 30 / height).unwrap(),
                    235,
                )
            };
            data.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Image::from_rgba(width, height, data).unwrap()
}

fn noise(width: u32, height: u32) -> Image {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let data = (0..width * height)
        .flat_map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let v = state.to_le_bytes();
            [v[0] | 1, v[1] | 1, v[2] | 1, 255]
        })
        .collect();
    Image::from_rgba(width, height, data).unwrap()
}

fn doc_with(width: u32, height: u32, edits: Vec<Edit>) -> Document {
    let mut doc = Document::new(BaseImage { width, height });
    let mut history = History::new();
    for edit in edits {
        history.apply(&mut doc, edit, None).unwrap();
    }
    doc
}

fn add(shape: Shape) -> Edit {
    Edit::Add { shape, style: None }
}

fn assert_matches_golden(name: &str, actual: &Image) {
    let path = golden_dir().join(format!("{name}.png"));
    if std::env::var_os("TELLSHOT_BLESS").is_some() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        tellshot_format::atomic_write(&path, &png_codec::encode(actual, Effort::Small).unwrap())
            .unwrap();
        return;
    }
    let Ok(bytes) = std::fs::read(&path) else {
        panic!("missing {}; run `cargo xtask bless`", path.display());
    };
    let expected = png_codec::decode(&bytes).unwrap();
    let same_size = (expected.width(), expected.height()) == (actual.width(), actual.height());
    let differing = if same_size {
        expected
            .as_bytes()
            .chunks_exact(4)
            .zip(actual.as_bytes().chunks_exact(4))
            .filter(|(e, a)| e.iter().zip(*a).any(|(x, y)| x.abs_diff(*y) > 2))
            .count()
    } else {
        usize::MAX
    };
    // Allow a sliver of anti-aliasing drift between CPU architectures, nothing more.
    let allowed = (actual.width() * actual.height()) as usize / 1000;
    if differing > allowed {
        let actual_path = golden_dir().join(format!("{name}.actual.png"));
        let _written = tellshot_format::atomic_write(
            &actual_path,
            &png_codec::encode(actual, Effort::Fast).unwrap(),
        );
        panic!(
            "{name}: {differing} pixels differ (allowed {allowed}); see {}",
            actual_path.display()
        );
    }
}

#[test]
fn every_annotation_kind() {
    let (w, h) = (360, 220);
    let blue = Style {
        stroke: palette::BLUE,
        fill: Some(palette::SKY_BLUE.with_alpha(0x55)),
        stroke_width: 3.0,
    };
    let doc = doc_with(
        w,
        h,
        vec![
            add(Shape::Blur { rect: Rect::new(10.0, 150.0, 110.0, 60.0), radius: 12.0 }),
            add(Shape::Pixelate { rect: Rect::new(130.0, 150.0, 100.0, 60.0), block_size: 10.0 }),
            add(Shape::Highlight { rect: Rect::new(10.0, 0.0, 150.0, 8.0) }),
            add(Shape::Rectangle { rect: Rect::new(20.0, 30.0, 120.0, 70.0), corner_radius: 10.0 }),
            Edit::Add {
                shape: Shape::Ellipse { rect: Rect::new(170.0, 30.0, 110.0, 70.0) },
                style: Some(blue),
            },
            add(Shape::Arrow { from: Point::new(330.0, 200.0), to: Point::new(250.0, 120.0) }),
            add(Shape::Line { from: Point::new(20.0, 120.0), to: Point::new(150.0, 125.0) }),
            add(Shape::Freehand {
                points: (0..30)
                    .map(|i| {
                        let t = f32::from(u8::try_from(i).unwrap()) / 29.0;
                        Point::new(t.mul_add(80.0, 260.0), (t * 9.0).sin().mul_add(12.0, 20.0))
                    })
                    .collect(),
            }),
            add(Shape::Step { center: Point::new(300.0, 60.0), radius: 14.0 }),
            add(Shape::Redact { rect: Rect::new(240.5, 150.25, 40.0, 20.0) }),
        ],
    );
    let out = render(&doc, &base(w, h), &Fonts::none()).unwrap();
    assert_matches_golden("every_annotation_kind", &out);
}

#[test]
fn redaction_destroys_every_covered_pixel() {
    let (w, h) = (64, 48);
    let rect = Rect::new(10.3, 7.6, 20.2, 9.1);
    let doc = doc_with(w, h, vec![add(Shape::Redact { rect })]);
    let out = render(&doc, &noise(w, h), &Fonts::none()).unwrap();
    // Every pixel the rectangle touches, even partially, must be pure opaque black.
    for y in 7..17 {
        for x in 10..31 {
            assert_eq!(out.pixel(x, y), Some([0, 0, 0, 255]), "pixel ({x},{y}) leaks");
        }
    }
    assert_ne!(out.pixel(9, 7), Some([0, 0, 0, 255]), "neighbours are untouched");
}

#[test]
fn effects_never_spread_redacted_pixels() {
    // Same image except inside the redaction: outside it, output must be identical, even where a
    // blur and a pixelate cell overlap the redaction edge.
    let (w, h) = (96, 64);
    let secret = Rect::new(30.0, 20.0, 20.0, 12.0);
    let doc = doc_with(
        w,
        h,
        vec![
            add(Shape::Redact { rect: secret }),
            add(Shape::Blur { rect: Rect::new(10.0, 10.0, 60.0, 40.0), radius: 16.0 }),
            add(Shape::Pixelate { rect: Rect::new(25.0, 15.0, 40.0, 30.0), block_size: 9.0 }),
        ],
    );
    let plain = Image::new(w, h, [128, 128, 128, 255]).unwrap();
    let noisy = {
        let mut img = plain.clone();
        let src = noise(w, h);
        for y in 20..32 {
            for x in 30..50 {
                let start = ((y * w + x) * 4) as usize;
                let px = src.pixel(x, y).unwrap();
                img.as_bytes_mut()[start..start + 4].copy_from_slice(&px);
            }
        }
        img
    };
    let a = render(&doc, &plain, &Fonts::none()).unwrap();
    let b = render(&doc, &noisy, &Fonts::none()).unwrap();
    assert_eq!(a, b, "output must not depend on redacted pixels anywhere");
}

#[test]
fn crop_sets_the_output_size() {
    let doc = doc_with(100, 80, vec![Edit::Crop { rect: Some(Rect::new(10.0, 20.0, 50.0, 30.0)) }]);
    let src = base(100, 80);
    let out = render(&doc, &src, &Fonts::none()).unwrap();
    assert_eq!((out.width(), out.height()), (50, 30), "cropped size");
    assert_eq!(out.pixel(0, 0), src.pixel(10, 20), "top-left comes from the crop origin");
}

#[test]
fn mismatched_base_is_rejected() {
    let doc = doc_with(10, 10, vec![]);
    assert!(render(&doc, &base(11, 10), &Fonts::none()).is_err(), "size mismatch");
}

#[test]
fn text_is_drawn_when_a_font_is_available() {
    let fonts = Fonts::system();
    if !fonts.is_loaded() {
        // Headless CI images may have no fonts at all; nothing to check then.
        return;
    }
    let doc = doc_with(
        200,
        60,
        vec![add(Shape::Text {
            origin: Point::new(10.0, 10.0),
            text: "Tellshot".into(),
            font_size: 28.0,
        })],
    );
    let src = Image::new(200, 60, [255, 255, 255, 255]).unwrap();
    let out = render(&doc, &src, &fonts).unwrap();
    let inked =
        out.as_bytes().chunks_exact(4).filter(|p| p.first().is_some_and(|&r| r < 250)).count();
    assert!(inked > 200, "text produced {inked} coloured pixels");
}
