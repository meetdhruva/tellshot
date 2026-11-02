//! `cargo xtask icons`: renders the SVG masters into every platform format.
//!
//! `tellshot.svg` is the full icon; `tellshot-small.svg` is a simplified drawing used at 32 px
//! and below, where the full design's fine strokes would blur together.
//!
//! Outputs (all committed, so builds never need this tool):
//! - `png/tellshot-<size>.png` for 16–1024 px (Linux hicolor theme, window icon, docs)
//! - `tellshot.ico` (Windows executables and installer)
//! - `tellshot.icns` (macOS app bundle)

use std::path::Path;

use resvg::{tiny_skia, usvg};

const PNG_SIZES: [u32; 10] = [16, 24, 32, 48, 64, 128, 256, 512, 1024, 22];
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const ICNS_SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

pub(crate) fn generate(root: &Path) -> Result<(), String> {
    let dir = root.join("assets").join("icons");
    let full = load(&dir.join("tellshot.svg"))?;
    let small = load(&dir.join("tellshot-small.svg"))?;
    let tree_for = |size: u32| if size <= SMALL_MAX { &small } else { &full };

    let png_dir = dir.join("png");
    std::fs::create_dir_all(&png_dir).map_err(|e| e.to_string())?;
    for size in PNG_SIZES {
        let pixmap = render(tree_for(size), size)?;
        let png = pixmap.encode_png().map_err(|e| e.to_string())?;
        write(&png_dir.join(format!("tellshot-{size}.png")), &png)?;
    }

    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let pixmap = render(tree_for(size), size)?;
        let image = ico::IconImage::from_rgba_data(size, size, unpremultiply(&pixmap));
        ico.add_entry(ico::IconDirEntry::encode(&image).map_err(|e| e.to_string())?);
    }
    let mut ico_bytes = Vec::new();
    ico.write(&mut ico_bytes).map_err(|e| e.to_string())?;
    write(&dir.join("tellshot.ico"), &ico_bytes)?;

    let mut icns = icns::IconFamily::new();
    for size in ICNS_SIZES {
        let pixmap = render(tree_for(size), size)?;
        let image =
            icns::Image::from_data(icns::PixelFormat::RGBA, size, size, unpremultiply(&pixmap))
                .map_err(|e| e.to_string())?;
        icns.add_icon(&image).map_err(|e| format!("icns {size}px: {e}"))?;
    }
    let mut icns_bytes = Vec::new();
    icns.write(&mut icns_bytes).map_err(|e| e.to_string())?;
    write(&dir.join("tellshot.icns"), &icns_bytes)?;

    let _ignored = std::io::Write::write_all(
        &mut std::io::stdout().lock(),
        format!(
            "wrote {} PNG files, tellshot.ico and tellshot.icns to {}\n",
            PNG_SIZES.len(),
            dir.display()
        )
        .as_bytes(),
    );
    Ok(())
}

/// Sizes at or below this use the simplified master.
const SMALL_MAX: u32 = 32;

fn load(path: &Path) -> Result<usvg::Tree, String> {
    let svg = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    usvg::Tree::from_data(&svg, &usvg::Options::default())
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

fn render(tree: &usvg::Tree, size: u32) -> Result<tiny_skia::Pixmap, String> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("invalid icon size")?;
    let scale = size_f32(size) / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(pixmap)
}

/// tiny-skia stores premultiplied alpha; ICO and ICNS expect straight alpha.
fn unpremultiply(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

fn size_f32(size: u32) -> f32 {
    f32::from(u16::try_from(size).unwrap_or(u16::MAX))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[expect(clippy::disallowed_methods, reason = "generated repository assets, not user data")]
    std::fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}
