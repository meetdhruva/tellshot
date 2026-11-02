//! Area effects: blur, pixelate and secure fill.
//!
//! Blur and pixelate are cosmetic. They can sometimes be reversed for text, so redaction must
//! use [`fill`], which overwrites the pixels outright.

use alloc::vec;
use alloc::vec::Vec;

use crate::{Image, PixelRect};

type Px = [u8; 4];

/// Overwrites every pixel in `rect` with `color` (alpha forced to 255 when `opaque`).
/// Used for redaction: the original pixels are gone from the buffer afterwards.
pub fn fill(img: &mut Image, rect: PixelRect, color: Px, opaque: bool) {
    let Some(r) = rect.clip(img.rect()) else {
        return;
    };
    let color = if opaque { [color[0], color[1], color[2], 255] } else { color };
    let (x0, x1) = (r.x as usize * 4, r.right() as usize * 4);
    for y in r.y..r.bottom() {
        if let Some(span) = img.row_mut(y).and_then(|row| row.get_mut(x0..x1)) {
            for px in span.chunks_exact_mut(4) {
                px.copy_from_slice(&color);
            }
        }
    }
}

/// Replaces each `block × block` cell inside `rect` with its average colour.
/// Cells are aligned to the rectangle's top-left corner; edge cells may be smaller.
pub fn pixelate(img: &mut Image, rect: PixelRect, block: u32) {
    let Some(r) = rect.clip(img.rect()) else {
        return;
    };
    let block = block.max(1);
    let mut cy = r.y;
    while cy < r.bottom() {
        let ch = block.min(r.bottom() - cy);
        let mut cx = r.x;
        while cx < r.right() {
            let cw = block.min(r.right() - cx);
            let cell = PixelRect { x: cx, y: cy, width: cw, height: ch };
            let avg = average(img, cell);
            fill(img, cell, avg, false);
            cx += cw;
        }
        cy += ch;
    }
}

/// A smaller copy that fits within `max_width × max_height`, by averaging whole-pixel blocks
/// (sharp text stays legible; no ringing). Returns a clone when the image already fits.
#[must_use]
pub fn downscale_to_fit(img: &Image, max_width: u32, max_height: u32) -> Image {
    let factor =
        img.width().div_ceil(max_width.max(1)).max(img.height().div_ceil(max_height.max(1))).max(1);
    if factor == 1 {
        return img.clone();
    }
    let (w, h) = (img.width().div_ceil(factor), img.height().div_ceil(factor));
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for by in 0..h {
        for bx in 0..w {
            let cell = PixelRect { x: bx * factor, y: by * factor, width: factor, height: factor };
            let px = cell.clip(img.rect()).map_or([0; 4], |c| average(img, c));
            data.extend_from_slice(&px);
        }
    }
    Image::from_rgba(w, h, data).unwrap_or_else(|_| img.clone())
}

fn average(img: &Image, r: PixelRect) -> Px {
    let mut sum = [0_u64; 4];
    let (x0, x1) = (r.x as usize * 4, r.right() as usize * 4);
    for y in r.y..r.bottom() {
        if let Some(span) = img.row(y).and_then(|row| row.get(x0..x1)) {
            for px in span.chunks_exact(4) {
                for (s, &c) in sum.iter_mut().zip(px) {
                    *s += u64::from(c);
                }
            }
        }
    }
    let n = (u64::from(r.width) * u64::from(r.height)).max(1);
    sum.map(|s| u8::try_from((s + n / 2) / n).unwrap_or(u8::MAX))
}

/// Gaussian-like blur of the pixels inside `rect`, using three box-blur passes.
///
/// Cost is linear in the area and independent of `sigma`. Only pixels inside `rect` are read,
/// so nothing outside the region bleeds in and the edges stay crisp.
///
/// Both passes stream memory row by row: the vertical pass slides a window of whole rows with
/// one accumulator per channel per column, instead of gathering columns (which thrashes cache).
pub fn blur(img: &mut Image, rect: PixelRect, sigma: f32) {
    let Some(area) = rect.clip(img.rect()) else {
        return;
    };
    if !(sigma.is_finite() && sigma > 0.0) {
        return;
    }
    let (width, height) = (area.width as usize, area.height as usize);
    let stride = width * 4;
    let (x0, x1) = (area.x as usize * 4, area.right() as usize * 4);
    let mut front: Vec<u8> = Vec::with_capacity(stride * height);
    for y in area.y..area.bottom() {
        if let Some(span) = img.row(y).and_then(|row| row.get(x0..x1)) {
            front.extend_from_slice(span);
        }
    }
    if front.len() != stride * height {
        return;
    }
    let mut back = vec![0_u8; front.len()];
    let mut acc = vec![0_u32; stride];
    for radius in box_radii(sigma) {
        for (src, dst) in front.chunks_exact(stride).zip(back.chunks_exact_mut(stride)) {
            box_row(src, dst, radius);
        }
        box_columns(&back, &mut front, stride, radius, &mut acc);
    }
    for (y, src) in (area.y..area.bottom()).zip(front.chunks_exact(stride)) {
        if let Some(span) = img.row_mut(y).and_then(|row| row.get_mut(x0..x1)) {
            span.copy_from_slice(src);
        }
    }
}

/// Divides by `window` with rounding, via a multiply by the rounded-up reciprocal.
/// Exact for every sum a box filter can produce (s ≤ 255 · window) while window < 4096.
#[derive(Clone, Copy)]
struct Divider {
    half: u64,
    inverse: u64,
}

impl Divider {
    fn new(window: usize) -> Self {
        let window = u64::try_from(window).unwrap_or(u64::MAX).max(1);
        Self { half: window / 2, inverse: (1_u64 << 32).div_ceil(window) }
    }

    fn apply(self, sum: u32) -> u8 {
        u8::try_from(((u64::from(sum) + self.half) * self.inverse) >> 32).unwrap_or(u8::MAX)
    }
}

/// Horizontal box filter of radius `r` over one RGBA row, clamped edges, O(n) running sums.
#[expect(clippy::indexing_slicing, reason = "pixel indices are clamped to 0..n before use")]
fn box_row(src: &[u8], dst: &mut [u8], r: usize) {
    let (src_px, _) = src.as_chunks::<4>();
    let (dst_px, _) = dst.as_chunks_mut::<4>();
    let n = src_px.len();
    if n == 0 || r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let last = n - 1;
    let div = Divider::new(2 * r + 1);
    let mut acc = [0_u32; 4];
    for k in 0..=2 * r {
        let px = src_px[k.saturating_sub(r).min(last)];
        for (a, &c) in acc.iter_mut().zip(&px) {
            *a += u32::from(c);
        }
    }
    for (i, out) in dst_px.iter_mut().enumerate() {
        *out = acc.map(|a| div.apply(a));
        let incoming = src_px[(i + r + 1).min(last)];
        let outgoing = src_px[i.saturating_sub(r)];
        for ((a, &inc), &out) in acc.iter_mut().zip(&incoming).zip(&outgoing) {
            *a = *a + u32::from(inc) - u32::from(out);
        }
    }
}

/// Vertical box filter of radius `r`: slides a window of whole rows, one accumulator per byte
/// column, so memory is read strictly row by row.
#[expect(clippy::indexing_slicing, reason = "row indices are clamped to 0..h; acc.len() == stride")]
fn box_columns(src: &[u8], dst: &mut [u8], stride: usize, r: usize, acc: &mut [u32]) {
    let h = src.len() / stride;
    if h == 0 || r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let last = h - 1;
    let div = Divider::new(2 * r + 1);
    let row = |y: usize| &src[y * stride..(y + 1) * stride];
    acc.fill(0);
    for k in 0..=2 * r {
        for (a, &v) in acc.iter_mut().zip(row(k.saturating_sub(r).min(last))) {
            *a += u32::from(v);
        }
    }
    for (y, out) in dst.chunks_exact_mut(stride).enumerate() {
        for (o, &a) in out.iter_mut().zip(acc.iter()) {
            *o = div.apply(a);
        }
        let incoming = row((y + r + 1).min(last));
        let outgoing = row(y.saturating_sub(r));
        for ((a, &i), &o) in acc.iter_mut().zip(incoming).zip(outgoing) {
            *a = *a + u32::from(i) - u32::from(o);
        }
    }
}

/// Radii of three box filters approximating a Gaussian of standard deviation `sigma`
/// (Wells 1986; Kutskir's "boxes for gauss").
fn box_radii(sigma: f32) -> [usize; 3] {
    // With n = 3 boxes: ideal width = sqrt(12σ²/n + 1) = sqrt(4σ² + 1), and the number of
    // boxes using the lower width is m = (12σ² − n·l² − 4n·l − 3n) / (−4l − 4).
    // Keeps every window below 4096, where Divider is provably exact (256·w² < 2³²).
    let sigma = sigma.min(1_000.0);
    let ideal = libm::sqrtf(libm::fmaf(4.0 * sigma, sigma, 1.0));
    let mut lower = libm::floorf(ideal);
    if libm::fmodf(lower, 2.0) == 0.0 {
        lower -= 1.0;
    }
    let lower = lower.max(1.0);
    let upper = lower + 2.0;
    let numerator = libm::fmaf(
        -3.0 * lower,
        lower,
        libm::fmaf(-12.0, lower, libm::fmaf(12.0 * sigma, sigma, -9.0)),
    );
    let m = libm::roundf(numerator / libm::fmaf(-4.0, lower, -4.0));
    let radius = |size: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "size is a small positive odd integer (sigma is clamped)"
        )]
        let r = ((size - 1.0) / 2.0) as usize;
        r
    };
    let pick = |i: f32| if i < m { radius(lower) } else { radius(upper) };
    [pick(0.0), pick(1.0), pick(2.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(w: u32, h: u32) -> Image {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        Image::from_rgba(w, h, data).unwrap()
    }

    #[test]
    fn fill_overwrites_and_forces_opacity() {
        let mut img = checkerboard(8, 8);
        let area = PixelRect { x: 2, y: 2, width: 3, height: 3 };
        fill(&mut img, area, [0, 0, 0, 10], true);
        for y in 2..5 {
            for x in 2..5 {
                assert_eq!(img.pixel(x, y), Some([0, 0, 0, 255]), "({x},{y}) redacted and opaque");
            }
        }
        assert_eq!(img.pixel(1, 1), Some([255, 255, 255, 255]), "outside untouched");
    }

    #[test]
    fn pixelate_averages_cells() {
        let mut img = checkerboard(4, 4);
        let all = img.rect();
        pixelate(&mut img, all, 2);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(img.pixel(x, y), Some([128, 128, 128, 255]), "({x},{y}) averaged");
            }
        }
    }

    #[test]
    fn blur_smooths_inside_and_leaves_outside_alone() {
        let mut img = checkerboard(40, 40);
        let area = PixelRect { x: 10, y: 10, width: 20, height: 20 };
        blur(&mut img, area, 4.0);
        let centre = img.pixel(20, 20).unwrap();
        assert!((120..=135).contains(&centre[0]), "centre is mid-grey, got {centre:?}");
        assert_eq!(img.pixel(5, 5), Some([255; 4]), "outside unchanged (white square)");
        assert_eq!(img.pixel(5, 6), Some([0, 0, 0, 255]), "outside unchanged (black square)");
    }

    #[test]
    fn blur_of_flat_colour_is_identity() {
        let mut img = Image::new(16, 16, [10, 20, 30, 255]).unwrap();
        let before = img.clone();
        let all = img.rect();
        blur(&mut img, all, 3.0);
        assert_eq!(img, before, "a box filter preserves a constant signal exactly");
    }

    #[test]
    fn degenerate_inputs_do_nothing() {
        let mut img = checkerboard(4, 4);
        let before = img.clone();
        let all = img.rect();
        blur(&mut img, all, f32::NAN);
        blur(&mut img, PixelRect { x: 10, y: 10, width: 5, height: 5 }, 2.0);
        pixelate(&mut img, PixelRect::default(), 3);
        assert_eq!(img, before, "unchanged");
    }

    #[test]
    fn downscale_fits_and_averages() {
        let img = checkerboard(100, 50);
        let small = downscale_to_fit(&img, 40, 40);
        assert_eq!((small.width(), small.height()), (34, 17), "factor 3, rounded up");
        assert_eq!(downscale_to_fit(&img, 200, 200), img, "already fits");
        let even = downscale_to_fit(&checkerboard(4, 4), 2, 2);
        assert_eq!(even.pixel(0, 0), Some([128, 128, 128, 255]), "2×2 blocks average to grey");
    }

    #[test]
    fn box_radii_grow_with_sigma() {
        let small = box_radii(1.0).iter().sum::<usize>();
        let large = box_radii(10.0).iter().sum::<usize>();
        assert!(large > small, "{large} > {small}");
        assert_eq!(box_radii(1e30), box_radii(1_000.0), "huge sigma is clamped");
    }
}
