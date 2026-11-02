//! Scrolling capture: find how far content moved between two viewport frames, then stitch.
//!
//! # Algorithm
//!
//! 1. Reduce every row to a 64-bit signature (FNV-1a over colour quantised to 5 bits per
//!    channel, so faint anti-aliasing noise does not break matches).
//! 2. Sticky header and footer: leading and trailing rows that are identical *at the same
//!    position* in both frames (toolbars, fixed table headers, status bars).
//! 3. For each candidate offset `d`, compare the remaining body rows of the previous frame,
//!    shifted by `d`, with the next frame. Only *informative* rows count (rows that differ from
//!    the row above), so blank margins cannot vote for a wrong offset.
//! 4. Accept the offset with the best score if it clears `min_confidence`; ties go to the
//!    largest overlap. Otherwise report [`ScrollError::NotFound`] so the UI can fall back to
//!    manual stitching instead of producing a silently wrong image.
//!
//! Cost is O(H²) signature comparisons per frame pair, which is a few million u64 compares for
//! a 4K-tall viewport.

use alloc::vec::Vec;
use core::fmt;

use crate::{Image, PixelRect};

/// Tuning knobs for overlap detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollOptions {
    /// Minimum number of overlapping body rows required to trust a match.
    pub min_overlap: u32,
    /// Minimum fraction (0–1) of informative overlapping rows that must match.
    pub min_confidence: f32,
    /// Largest fraction of the frame height that may be classed as sticky header or footer.
    pub max_fixed_fraction: f32,
    /// Refuse to grow the stitched image beyond this many rows.
    pub max_height: u32,
    /// Refuse to grow the stitched image beyond this many pixels (matches the project limit).
    pub max_pixels: u64,
}

impl Default for ScrollOptions {
    fn default() -> Self {
        Self {
            min_overlap: 16,
            min_confidence: 0.95,
            max_fixed_fraction: 0.4,
            max_height: 1 << 20,
            max_pixels: 1 << 29,
        }
    }
}

/// The relationship between two consecutive frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlap {
    /// How many rows the content moved up between the frames.
    pub offset: u32,
    /// Rows at the top that did not scroll.
    pub header: u32,
    /// Rows at the bottom that did not scroll.
    pub footer: u32,
    /// Fraction of informative overlapping rows that matched.
    pub confidence: f32,
}

/// Why two frames could not be related.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollError {
    /// The frames have different sizes.
    SizeMismatch,
    /// Nothing moved between the frames.
    NoMovement,
    /// No offset matched confidently (content changed, or scrolled too far in one step).
    NotFound,
    /// The stitched image would exceed [`ScrollOptions::max_height`].
    TooTall,
}

impl fmt::Display for ScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SizeMismatch => "frames have different sizes",
            Self::NoMovement => "the content did not move",
            Self::NotFound => "could not find a confident overlap; scroll less per step",
            Self::TooTall => "the stitched image is too tall",
        })
    }
}

impl core::error::Error for ScrollError {}

/// One 64-bit signature per row.
#[must_use]
pub fn row_signatures(img: &Image) -> Vec<u64> {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    (0..img.height())
        .map(|y| {
            img.row(y).map_or(0, |row| {
                row.chunks_exact(4).fold(FNV_OFFSET, |h, px| {
                    px.iter()
                        .take(3)
                        .fold(h, |h, &c| (h ^ u64::from(c >> 3)).wrapping_mul(FNV_PRIME))
                })
            })
        })
        .collect()
}

/// Detects sticky header/footer and the scroll offset between `prev` and `next`.
///
/// # Errors
/// See [`ScrollError`].
pub fn find_overlap(
    prev: &Image,
    next: &Image,
    opts: &ScrollOptions,
) -> Result<Overlap, ScrollError> {
    if prev.width() != next.width() || prev.height() != next.height() {
        return Err(ScrollError::SizeMismatch);
    }
    let (a, b) = (row_signatures(prev), row_signatures(next));
    if a == b {
        return Err(ScrollError::NoMovement);
    }
    let h = a.len();
    let max_fixed = fraction_of(h, opts.max_fixed_fraction);
    let header = a.iter().zip(&b).take_while(|(x, y)| x == y).count().min(max_fixed);
    let footer =
        a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count().min(max_fixed);
    best_offset(&a, &b, header, footer, opts)
}

/// Like [`find_overlap`] but with a known header and footer (used after the first frame pair,
/// so every frame is cut at the same rows).
///
/// # Errors
/// See [`ScrollError`].
pub fn find_offset(
    prev: &Image,
    next: &Image,
    header: u32,
    footer: u32,
    opts: &ScrollOptions,
) -> Result<Overlap, ScrollError> {
    if prev.width() != next.width() || prev.height() != next.height() {
        return Err(ScrollError::SizeMismatch);
    }
    let (a, b) = (row_signatures(prev), row_signatures(next));
    if a == b {
        return Err(ScrollError::NoMovement);
    }
    best_offset(&a, &b, header as usize, footer as usize, opts)
}

fn best_offset(
    a: &[u64],
    b: &[u64],
    header: usize,
    footer: usize,
    opts: &ScrollOptions,
) -> Result<Overlap, ScrollError> {
    let end = a.len().saturating_sub(footer);
    let (Some(body_a), Some(body_b)) = (a.get(header..end), b.get(header..end)) else {
        return Err(ScrollError::NotFound);
    };
    if body_a == body_b {
        return Err(ScrollError::NoMovement);
    }
    let len = body_a.len();
    let min_overlap = opts.min_overlap.max(1) as usize;
    if len <= min_overlap {
        return Err(ScrollError::NotFound);
    }
    // Rows of `next` that carry information: they differ from the row above.
    let informative: Vec<bool> =
        body_b.iter().enumerate().map(|(i, s)| i == 0 || body_b.get(i - 1) != Some(s)).collect();

    let mut best: Option<(usize, f32)> = None;
    for d in 1..=len - min_overlap {
        let n = len - d;
        let (mut matched, mut counted) = (0_usize, 0_usize);
        for ((sa, sb), &info) in body_a.iter().skip(d).zip(body_b).zip(&informative).take(n) {
            if info {
                counted += 1;
                matched += usize::from(sa == sb);
            }
        }
        if counted < 4 {
            continue;
        }
        let score = ratio(matched, counted);
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((d, score));
        }
    }
    match best {
        Some((d, confidence)) if confidence >= opts.min_confidence => Ok(Overlap {
            offset: to_u32(d),
            header: to_u32(header),
            footer: to_u32(footer),
            confidence,
        }),
        _ => Err(ScrollError::NotFound),
    }
}

/// Accumulates viewport frames of a scrolling page into one tall image.
#[derive(Debug, Clone)]
pub struct Stitcher {
    opts: ScrollOptions,
    first: Option<Image>,
    last: Option<Image>,
    cut: Option<(u32, u32)>,
    body: Vec<u8>,
    body_rows: u32,
}

impl Stitcher {
    /// A new, empty stitcher.
    #[must_use]
    pub const fn new(opts: ScrollOptions) -> Self {
        Self { opts, first: None, last: None, cut: None, body: Vec::new(), body_rows: 0 }
    }

    /// Adds the next frame. Returns how many new rows it contributed (0 when nothing moved).
    ///
    /// # Errors
    /// [`ScrollError::NotFound`] when the frame cannot be placed; the stitcher is unchanged and
    /// the caller may retry with a frame that scrolled less.
    pub fn push(&mut self, frame: Image) -> Result<u32, ScrollError> {
        let Some(prev) = &self.last else {
            self.first = Some(frame.clone());
            self.last = Some(frame);
            return Ok(0);
        };
        let overlap = match self.cut {
            None => find_overlap(prev, &frame, &self.opts),
            Some((header, footer)) => find_offset(prev, &frame, header, footer, &self.opts),
        };
        let overlap = match overlap {
            Ok(o) => o,
            Err(ScrollError::NoMovement) => return Ok(0),
            Err(e) => return Err(e),
        };
        let height = frame.height();
        let fixed = overlap.header + overlap.footer;
        if self.cut.is_none() {
            // First successful pair: the body of the first frame starts the output.
            let Some(first) = &self.first else {
                return Err(ScrollError::NotFound);
            };
            let body = first.crop(PixelRect {
                x: 0,
                y: overlap.header,
                width: first.width(),
                height: height.saturating_sub(fixed),
            });
            let body = body.ok_or(ScrollError::NotFound)?;
            self.body_rows = body.height();
            self.body = body.into_bytes();
            self.cut = Some((overlap.header, overlap.footer));
        }
        let new_rows = overlap.offset;
        let grown = self.body_rows.saturating_add(new_rows).saturating_add(fixed);
        if grown > self.opts.max_height
            || u64::from(grown) * u64::from(frame.width()) > self.opts.max_pixels
        {
            return Err(ScrollError::TooTall);
        }
        let revealed = frame
            .crop(PixelRect {
                x: 0,
                y: height.saturating_sub(overlap.footer).saturating_sub(new_rows),
                width: frame.width(),
                height: new_rows,
            })
            .ok_or(ScrollError::NotFound)?;
        self.body.extend_from_slice(revealed.as_bytes());
        self.body_rows += new_rows;
        self.last = Some(frame);
        Ok(new_rows)
    }

    /// The stitched image: header of the first frame, accumulated body, footer of the last.
    #[must_use]
    pub fn finish(self) -> Option<Image> {
        let last = self.last?;
        let Some((header, footer)) = self.cut else {
            return Some(last);
        };
        let first = self.first?;
        let width = last.width();
        let mut data = Vec::new();
        data.extend_from_slice(
            first
                .crop(PixelRect { x: 0, y: 0, width, height: header })
                .map(Image::into_bytes)
                .unwrap_or_default()
                .as_slice(),
        );
        data.extend_from_slice(&self.body);
        let foot_y = last.height().saturating_sub(footer);
        if let Some(foot) = last.crop(PixelRect { x: 0, y: foot_y, width, height: footer }) {
            data.extend_from_slice(foot.as_bytes());
        }
        let rows = u32::try_from(data.len() / (width as usize * 4)).ok()?;
        Image::from_rgba(width, rows, data).ok()
    }
}

fn fraction_of(n: usize, fraction: f32) -> usize {
    let f = fraction.clamp(0.0, 1.0);
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "row counts are far below 2^24 and the product is clamped to 0..=n"
    )]
    let rows = (n as f32 * f) as usize;
    rows.min(n)
}

fn ratio(num: usize, den: usize) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "row counts are far below 2^24")]
    let r = num as f32 / den.max(1) as f32;
    r
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 48;
    const HEADER: u32 = 24;
    const FOOTER: u32 = 16;
    const VIEW: u32 = 200;

    /// A deterministic pseudo-random row; some rows are blank to mimic real pages.
    fn page_row(y: u32) -> Vec<u8> {
        if (300..340).contains(&y) {
            return [250_u8, 250, 250, 255].repeat(W as usize);
        }
        let mut state = u64::from(y).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        (0..W)
            .flat_map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let v = state.to_le_bytes();
                [v[0], v[1], v[2], 255]
            })
            .collect()
    }

    fn solid_rows(rows: u32, v: u8) -> Vec<u8> {
        [v, v / 2, 255 - v, 255].repeat((W * rows) as usize)
    }

    fn frame(scroll: u32) -> Image {
        let mut data = solid_rows(HEADER, 40);
        for y in scroll..scroll + VIEW {
            data.extend(page_row(y));
        }
        data.extend(solid_rows(FOOTER, 200));
        Image::from_rgba(W, HEADER + VIEW + FOOTER, data).unwrap()
    }

    fn expected(last_scroll: u32) -> Image {
        let mut data = solid_rows(HEADER, 40);
        for y in 0..last_scroll + VIEW {
            data.extend(page_row(y));
        }
        data.extend(solid_rows(FOOTER, 200));
        Image::from_rgba(W, HEADER + last_scroll + VIEW + FOOTER, data).unwrap()
    }

    #[test]
    fn detects_offset_and_sticky_bars() {
        let o = find_overlap(&frame(0), &frame(37), &ScrollOptions::default()).unwrap();
        assert_eq!((o.offset, o.header, o.footer), (37, HEADER, FOOTER), "{o:?}");
        assert!(o.confidence >= 0.99, "confidence {}", o.confidence);
    }

    #[test]
    fn stitches_a_page_exactly_including_blank_regions_and_pauses() {
        let mut s = Stitcher::new(ScrollOptions::default());
        let mut added = 0;
        for pos in [0, 70, 150, 150, 260, 330, 420] {
            added += s.push(frame(pos)).unwrap();
        }
        assert_eq!(added, 420, "total new rows");
        let out = s.finish().unwrap();
        assert_eq!(out, expected(420), "pixel-exact stitch");
    }

    #[test]
    fn refuses_to_guess_when_frames_do_not_overlap() {
        let err = find_overlap(&frame(0), &frame(500), &ScrollOptions::default());
        assert_eq!(err, Err(ScrollError::NotFound), "no shared rows");
        let mut s = Stitcher::new(ScrollOptions::default());
        s.push(frame(0)).unwrap();
        assert_eq!(s.push(frame(500)), Err(ScrollError::NotFound), "push fails cleanly");
        assert_eq!(s.push(frame(60)).unwrap(), 60, "and recovers with a closer frame");
    }

    #[test]
    fn single_frame_is_returned_unchanged() {
        let mut s = Stitcher::new(ScrollOptions::default());
        s.push(frame(10)).unwrap();
        assert_eq!(s.finish(), Some(frame(10)), "unchanged");
    }
}
