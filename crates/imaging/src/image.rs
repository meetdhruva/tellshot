//! The RGBA8 image buffer shared by capture, rendering and file I/O.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// An owned RGBA8 image with straight (non-premultiplied) alpha, rows packed with no padding.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// Why an [`Image`] could not be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    /// Width or height is zero, or the byte size overflows.
    BadDimensions,
    /// The buffer length is not `width * height * 4`.
    LengthMismatch,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadDimensions => f.write_str("image dimensions are zero or too large"),
            Self::LengthMismatch => f.write_str("pixel buffer length does not match dimensions"),
        }
    }
}

impl core::error::Error for ImageError {}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Image {
    /// A new image filled with `color`.
    ///
    /// # Errors
    /// [`ImageError::BadDimensions`] when a dimension is zero or the size overflows `usize`.
    pub fn new(width: u32, height: u32, color: [u8; 4]) -> Result<Self, ImageError> {
        let len = byte_len(width, height)?;
        let mut data = vec![0; len];
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&color);
        }
        Ok(Self { width, height, data })
    }

    /// Wraps an existing RGBA8 buffer.
    ///
    /// # Errors
    /// When dimensions are zero or the buffer length does not match.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Result<Self, ImageError> {
        if byte_len(width, height)? != data.len() {
            return Err(ImageError::LengthMismatch);
        }
        Ok(Self { width, height, data })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The whole image as a rectangle.
    #[must_use]
    pub const fn rect(&self) -> PixelRect {
        PixelRect { x: 0, y: 0, width: self.width, height: self.height }
    }

    /// Raw RGBA8 bytes, row by row.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Mutable raw RGBA8 bytes.
    #[must_use]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Consumes the image, returning its bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Bytes per row.
    #[must_use]
    pub const fn stride(&self) -> usize {
        self.width as usize * 4
    }

    /// One row of pixels, or `None` when `y` is out of range.
    #[must_use]
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        self.data.chunks_exact(self.stride()).nth(y as usize)
    }

    /// One mutable row of pixels.
    #[must_use]
    pub fn row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        let stride = self.stride();
        self.data.chunks_exact_mut(stride).nth(y as usize)
    }

    /// The pixel at `(x, y)`.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width {
            return None;
        }
        let start = x as usize * 4;
        let px = self.row(y)?.get(start..start + 4)?;
        px.try_into().ok()
    }

    /// A copy of the area `rect` (clipped to the image), or `None` when the result is empty.
    #[must_use]
    pub fn crop(&self, rect: PixelRect) -> Option<Self> {
        let r = rect.clip(self.rect())?;
        let (x0, x1) = (r.x as usize * 4, r.right() as usize * 4);
        let mut data = Vec::with_capacity(r.width as usize * r.height as usize * 4);
        for y in r.y..r.bottom() {
            data.extend_from_slice(self.row(y)?.get(x0..x1)?);
        }
        Self::from_rgba(r.width, r.height, data).ok()
    }
}

/// An integer pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelRect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl PixelRect {
    /// Right edge (exclusive), saturating.
    #[must_use]
    pub const fn right(self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// Bottom edge (exclusive), saturating.
    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// The part of `self` inside `bounds`, or `None` when they do not overlap.
    #[must_use]
    pub fn clip(self, bounds: Self) -> Option<Self> {
        let x0 = self.x.max(bounds.x);
        let y0 = self.y.max(bounds.y);
        let x1 = self.right().min(bounds.right());
        let y1 = self.bottom().min(bounds.bottom());
        let width = x1.checked_sub(x0).filter(|w| *w > 0)?;
        let height = y1.checked_sub(y0).filter(|h| *h > 0)?;
        Some(Self { x: x0, y: y0, width, height })
    }

    /// Converts a floating-point rectangle (document pixels) to the covering pixel rectangle,
    /// clipped to `bounds`. Partially covered pixels are included, so a redaction never
    /// leaves a sliver of the original visible.
    #[must_use]
    pub fn covering(x: f32, y: f32, width: f32, height: f32, bounds: Self) -> Option<Self> {
        if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()) {
            return None;
        }
        let to_px = |v: f32| -> u32 {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to 0..=2^24 before the cast; images are at most 2^20 pixels wide"
            )]
            let px = v.clamp(0.0, 16_777_216.0) as u32;
            px
        };
        let x0 = to_px(libm::floorf(x.min(x + width)));
        let y0 = to_px(libm::floorf(y.min(y + height)));
        let x1 = to_px(libm::ceilf(x.max(x + width)));
        let y1 = to_px(libm::ceilf(y.max(y + height)));
        Self { x: x0, y: y0, width: x1.saturating_sub(x0), height: y1.saturating_sub(y0) }
            .clip(bounds)
    }
}

fn byte_len(width: u32, height: u32) -> Result<usize, ImageError> {
    if width == 0 || height == 0 {
        return Err(ImageError::BadDimensions);
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or(ImageError::BadDimensions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_validates_sizes() {
        assert_eq!(Image::new(0, 5, [0; 4]), Err(ImageError::BadDimensions), "zero width");
        assert_eq!(
            Image::from_rgba(2, 2, vec![0; 15]),
            Err(ImageError::LengthMismatch),
            "short buffer"
        );
        let img = Image::new(3, 2, [1, 2, 3, 4]).unwrap();
        assert_eq!(img.pixel(2, 1), Some([1, 2, 3, 4]), "last pixel");
        assert_eq!(img.pixel(3, 0), None, "x out of range");
        assert_eq!(img.pixel(0, 2), None, "y out of range");
    }

    #[test]
    fn covering_includes_partial_pixels_and_clips() {
        let bounds = PixelRect { x: 0, y: 0, width: 100, height: 100 };
        let r = PixelRect::covering(1.5, 2.2, 3.0, 1.0, bounds).unwrap();
        assert_eq!(r, PixelRect { x: 1, y: 2, width: 4, height: 2 }, "rounds outward");
        let clipped = PixelRect::covering(-10.0, 90.0, 20.0, 50.0, bounds).unwrap();
        assert_eq!(clipped, PixelRect { x: 0, y: 90, width: 10, height: 10 }, "clipped");
        assert_eq!(PixelRect::covering(f32::NAN, 0.0, 1.0, 1.0, bounds), None, "NaN");
        assert_eq!(PixelRect::covering(200.0, 0.0, 1.0, 1.0, bounds), None, "outside");
    }

    #[test]
    fn crop_copies_the_area() {
        let mut img = Image::new(4, 4, [0; 4]).unwrap();
        if let Some(row) = img.row_mut(2) {
            row.fill(9);
        }
        let c = img.crop(PixelRect { x: 1, y: 1, width: 2, height: 2 }).unwrap();
        assert_eq!((c.width(), c.height()), (2, 2), "size");
        assert_eq!(c.pixel(0, 1), Some([9; 4]), "row 2 of the source is row 1 of the crop");
    }
}
