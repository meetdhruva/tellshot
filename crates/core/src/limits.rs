//! Hard limits applied to every document and edit.
//!
//! Documents arrive from files and from AI agents, so every size is bounded. Limits are
//! generous for real use. The image limit still allows a 2 GiB buffer (more during rendering),
//! so callers serving untrusted input should apply a lower, deployment-specific cap.

/// Upper bounds enforced by [`crate::Document::validate`] and [`crate::Edit`] resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits;

impl Limits {
    /// Maximum width or height of a base image, in pixels (long scrolling captures included).
    pub const MAX_IMAGE_DIMENSION: u32 = 1 << 20;
    /// Maximum total pixels in a base image (512 Mpx ≈ 2 GiB of RGBA).
    pub const MAX_IMAGE_PIXELS: u64 = 1 << 29;
    /// Maximum number of annotation objects in one document.
    pub const MAX_OBJECTS: usize = 10_000;
    /// Maximum characters in one text object.
    pub const MAX_TEXT_CHARS: usize = 10_000;
    /// Maximum points in one freehand stroke.
    pub const MAX_FREEHAND_POINTS: usize = 100_000;
    /// Maximum absolute value of any coordinate, in document pixels.
    pub const MAX_COORDINATE: f32 = 4_194_304.0;
    /// Maximum stroke width, font size, blur radius or similar scalar.
    pub const MAX_SCALAR: f32 = 4_096.0;
    /// Maximum number of entries kept on the undo stack.
    pub const MAX_UNDO_DEPTH: usize = 1_000;
}
