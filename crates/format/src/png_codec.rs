//! PNG encoding and decoding into [`Image`], with size limits for untrusted files.

use std::io::Cursor;

use tellshot_core::Limits;
use tellshot_imaging::Image;

use crate::{FormatError, Result};

/// How hard the encoder works. Captures favour speed; exports favour size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effort {
    /// Fast encode for capture-time saves (the hot path after the hotkey).
    #[default]
    Fast,
    /// Smaller files for export and sharing.
    Small,
}

/// Encodes an RGBA8 image as PNG.
///
/// # Errors
/// [`FormatError::Png`] if the encoder fails.
pub fn encode(img: &Image, effort: Effort) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width(), img.height());
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(match effort {
            Effort::Fast => png::Compression::Fast,
            Effort::Small => png::Compression::High,
        });
        let mut writer = enc.write_header().map_err(|e| FormatError::Png(e.to_string()))?;
        writer.write_image_data(img.as_bytes()).map_err(|e| FormatError::Png(e.to_string()))?;
        writer.finish().map_err(|e| FormatError::Png(e.to_string()))?;
    }
    Ok(out)
}

/// Decodes a PNG of any colour type into RGBA8.
///
/// # Errors
/// [`FormatError::Png`] for malformed data, [`FormatError::Invalid`] when the image exceeds
/// [`Limits`].
pub fn decode(bytes: &[u8]) -> Result<Image> {
    let max_bytes = usize::try_from(Limits::MAX_IMAGE_PIXELS * 4).unwrap_or(usize::MAX);
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: max_bytes });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| FormatError::Png(e.to_string()))?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0
        || height == 0
        || width > Limits::MAX_IMAGE_DIMENSION
        || height > Limits::MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > Limits::MAX_IMAGE_PIXELS
    {
        return Err(FormatError::Invalid(format!("image size {width}×{height} is out of range")));
    }
    let size =
        reader.output_buffer_size().ok_or_else(|| FormatError::Png("image too large".into()))?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| FormatError::Png(e.to_string()))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            buf.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect()
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            buf.as_chunks::<2>().0.iter().flat_map(|&[g, a]| [g, g, g, a]).collect()
        }
        png::ColorType::Indexed => {
            return Err(FormatError::Png("indexed colour was not expanded".into()));
        }
    };
    Image::from_rgba(info.width, info.height, rgba).map_err(|e| FormatError::Png(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_pixels() {
        let data: Vec<u8> = (0..16 * 9 * 4).map(|i| u8::try_from(i % 251).unwrap()).collect();
        let img = Image::from_rgba(16, 9, data).unwrap();
        for effort in [Effort::Fast, Effort::Small] {
            let back = decode(&encode(&img, effort).unwrap()).unwrap();
            assert_eq!(back, img, "{effort:?}");
        }
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode(b"").is_err(), "empty");
        assert!(decode(b"\x89PNG\r\n\x1a\nnot really").is_err(), "truncated");
    }
}
