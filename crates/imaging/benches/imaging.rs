//! Benchmarks for the hot paths. Run with `cargo bench -p tellshot-imaging`.
//! Budget: blurring a full 1080p frame should fit in one frame (16 ms). Measured on a 2-vCPU
//! VM: about 60 ms, so blur is not yet within budget. Planned: blur a downscaled copy when sigma is
//! large, and SIMD via `wide` or `std::simd`. Track progress against this benchmark.
#![expect(clippy::expect_used, reason = "a benchmark should abort loudly if its setup fails")]

use divan::{Bencher, black_box};
use tellshot_imaging::{Image, effects, scroll};

fn main() {
    divan::main();
}

fn noise(width: u32, height: u32) -> Image {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let data = (0..width * height)
        .flat_map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let v = state.to_le_bytes();
            [v[0], v[1], v[2], 255]
        })
        .collect();
    Image::from_rgba(width, height, data).expect("valid size")
}

#[divan::bench(args = [2.0, 8.0, 32.0])]
fn blur_1080p(bencher: Bencher<'_, '_>, sigma: f32) {
    let img = noise(1920, 1080);
    bencher.with_inputs(|| img.clone()).bench_local_values(|mut img| {
        let rect = img.rect();
        effects::blur(&mut img, rect, sigma);
        black_box(img)
    });
}

#[divan::bench]
fn pixelate_1080p(bencher: Bencher<'_, '_>) {
    let img = noise(1920, 1080);
    bencher.with_inputs(|| img.clone()).bench_local_values(|mut img| {
        let rect = img.rect();
        effects::pixelate(&mut img, rect, 12);
        black_box(img)
    });
}

#[divan::bench]
fn scroll_overlap_1080p(bencher: Bencher<'_, '_>) {
    let tall = noise(1920, 1400);
    let a = tall.crop(tellshot_imaging::PixelRect { x: 0, y: 0, width: 1920, height: 1080 });
    let b = tall.crop(tellshot_imaging::PixelRect { x: 0, y: 120, width: 1920, height: 1080 });
    let (a, b) = (a.expect("crop"), b.expect("crop"));
    let opts = scroll::ScrollOptions::default();
    bencher.bench_local(|| black_box(scroll::find_overlap(&a, &b, &opts)));
}
