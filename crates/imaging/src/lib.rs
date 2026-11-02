//! Pixel algorithms for Tellshot. `no_std`: pure functions over RGBA buffers, no I/O.
//!
//! * [`Image`]: an owned, tightly packed RGBA8 buffer (straight alpha).
//! * [`effects`]: blur, pixelate and secure fill, applied to a [`PixelRect`].
//! * [`scroll`]: overlap detection and stitching for scrolling captures.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod effects;
mod image;
pub mod scroll;

pub use image::{Image, ImageError, PixelRect};
