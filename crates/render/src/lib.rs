//! Flattens a [`Document`] over its base image into final pixels.
//!
//! This is the *export* renderer: deterministic, CPU-only (tiny-skia) and safe to run headless
//! in the CLI, in tests and on servers. The editor uses it for its canvas today; a GPU renderer
//! for interactive editing can replace that later without changing export output.
//!
//! # Order of operations
//!
//! 1. Copy the base image and overwrite every redacted area, so no effect can spread
//!    redacted pixels.
//! 2. Apply raster effects (blur, pixelate) in z-order. They affect base pixels only, never
//!    annotations, so an arrow over a blurred area stays sharp.
//! 3. Draw annotations in z-order. Redactions overwrite whole pixels with opaque black, with no
//!    anti-aliased edge that could leak a sliver of what is underneath.
//! 4. Crop.

mod text;
mod vector;

use tellshot_core::{Document, Shape};
use tellshot_imaging::{Image, PixelRect, effects};
use tiny_skia::{IntSize, Pixmap};

pub use text::Fonts;

/// Why rendering failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// The base image does not have the size the document expects.
    #[error("base image is {actual:?} but the document expects {expected:?}")]
    BaseSizeMismatch {
        /// Size recorded in the document.
        expected: (u32, u32),
        /// Size of the image supplied.
        actual: (u32, u32),
    },
    /// The image is too large for the rasteriser.
    #[error("image is too large to render")]
    TooLarge,
    /// The crop leaves nothing to show.
    #[error("the crop is empty")]
    EmptyCrop,
}

/// Renders `doc` over `base` and returns the flattened, cropped image.
///
/// `fonts` supplies glyphs for text and step numbers; with [`Fonts::none`] text is skipped.
///
/// # Errors
/// See [`RenderError`].
pub fn render(doc: &Document, base: &Image, fonts: &Fonts) -> Result<Image, RenderError> {
    let expected = (doc.base().width, doc.base().height);
    let actual = (base.width(), base.height());
    if expected != actual {
        return Err(RenderError::BaseSizeMismatch { expected, actual });
    }

    // 1. Copy the base and destroy redacted pixels first, so no later effect (a blur or a
    //    pixelate cell overlapping the redaction) can smear secret pixels outside the black box.
    let mut img = base.clone();
    for object in doc.objects() {
        if let Shape::Redact { rect } = &object.shape
            && let Some(area) = cover(rect, &img)
        {
            effects::fill(&mut img, area, [0, 0, 0, 255], true);
        }
    }

    // 2. Raster effects in z-order.
    for object in doc.objects() {
        match &object.shape {
            Shape::Blur { rect, radius } => {
                if let Some(area) = cover(rect, &img) {
                    effects::blur(&mut img, area, radius / 2.0);
                }
            }
            Shape::Pixelate { rect, block_size } => {
                if let Some(area) = cover(rect, &img) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "block_size is validated to 1..=Limits::MAX_SCALAR"
                    )]
                    let block = block_size.round().max(1.0) as u32;
                    effects::pixelate(&mut img, area, block);
                }
            }
            Shape::Arrow { .. }
            | Shape::Line { .. }
            | Shape::Rectangle { .. }
            | Shape::Ellipse { .. }
            | Shape::Text { .. }
            | Shape::Step { .. }
            | Shape::Highlight { .. }
            | Shape::Redact { .. }
            | Shape::Freehand { .. } => {}
        }
    }

    // 3. Annotations.
    let mut pixmap = to_pixmap(img)?;
    for object in doc.objects() {
        let step = doc.step_number(object.id);
        vector::draw(&mut pixmap, object, step, fonts);
    }
    let img = from_pixmap(&pixmap)?;

    // 4. Crop.
    let visible = doc.visible_rect();
    let area = PixelRect::covering(visible.x, visible.y, visible.width, visible.height, img.rect())
        .ok_or(RenderError::EmptyCrop)?;
    if area == img.rect() { Ok(img) } else { img.crop(area).ok_or(RenderError::EmptyCrop) }
}

fn cover(rect: &tellshot_core::Rect, img: &Image) -> Option<PixelRect> {
    PixelRect::covering(rect.x, rect.y, rect.width, rect.height, img.rect())
}

/// Straight RGBA → premultiplied pixmap.
fn to_pixmap(img: Image) -> Result<Pixmap, RenderError> {
    let size = IntSize::from_wh(img.width(), img.height()).ok_or(RenderError::TooLarge)?;
    let mut data = img.into_bytes();
    for px in data.chunks_exact_mut(4) {
        if let [r, g, b, a] = px
            && *a != 255
        {
            let alpha = u16::from(*a);
            *r = premultiply(*r, alpha);
            *g = premultiply(*g, alpha);
            *b = premultiply(*b, alpha);
        }
    }
    Pixmap::from_vec(data, size).ok_or(RenderError::TooLarge)
}

/// Premultiplied pixmap → straight RGBA.
fn from_pixmap(pixmap: &Pixmap) -> Result<Image, RenderError> {
    let mut data = pixmap.data().to_vec();
    for px in data.chunks_exact_mut(4) {
        if let [r, g, b, a] = px
            && *a != 255
            && *a != 0
        {
            let alpha = u16::from(*a);
            *r = unpremultiply(*r, alpha);
            *g = unpremultiply(*g, alpha);
            *b = unpremultiply(*b, alpha);
        }
    }
    Image::from_rgba(pixmap.width(), pixmap.height(), data).map_err(|_| RenderError::TooLarge)
}

fn premultiply(c: u8, a: u16) -> u8 {
    u8::try_from((u16::from(c) * a + 127) / 255).unwrap_or(u8::MAX)
}

fn unpremultiply(c: u8, a: u16) -> u8 {
    u8::try_from(((u16::from(c) * 255) + a / 2) / a).unwrap_or(u8::MAX)
}
