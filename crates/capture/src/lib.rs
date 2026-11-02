//! Screen capture behind one trait, [`CaptureBackend`].
//!
//! Every OS needs its own API, so each lives in its own module and nothing outside this crate
//! sees platform types. Coordinates here are *physical* pixels in the desktop's coordinate
//! space (monitors may have negative origins); the editor converts to document space.
//!
//! | backend     | platforms                     | notes                                      |
//! |-------------|-------------------------------|--------------------------------------------|
//! | `x11`       | Linux/BSD on X11 or XWayland  | RandR monitors, `GetImage`                 |
//! | `xcap`      | Windows, macOS                | interim; native WGC/ScreenCaptureKit next  |
//! | `synthetic` | everywhere                    | deterministic test pattern for CI & agents |
//!
//! Wayland sessions need the xdg-desktop-portal backend (ScreenCast + PipeWire), which is on the
//! roadmap; until then [`detect`] reports it clearly instead of returning black frames.

mod synthetic;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;
#[cfg(any(windows, target_os = "macos"))]
mod xcap_backend;

use std::fmt;

use serde::{Deserialize, Serialize};
use tellshot_imaging::Image;

pub use synthetic::SyntheticBackend;

/// A rectangle in physical desktop pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct PhysicalRect {
    /// Left edge; negative for monitors left of the primary.
    pub x: i32,
    /// Top edge; negative for monitors above the primary.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl PhysicalRect {
    /// The overlap of two rectangles, or `None`.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x0 = i64::from(self.x).max(i64::from(other.x));
        let y0 = i64::from(self.y).max(i64::from(other.y));
        let x1 = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let y1 = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Self {
            x: i32::try_from(x0).ok()?,
            y: i32::try_from(y0).ok()?,
            width: u32::try_from(x1 - x0).ok()?,
            height: u32::try_from(y1 - y0).ok()?,
        })
    }
}

/// Opaque, backend-specific identifier of a capture target. Stable for the session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TargetId(pub String);

impl fmt::Display for TargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a target is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// A whole monitor.
    Monitor,
    /// A top-level window.
    Window,
}

/// Something that can be captured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// Identifier to pass to [`CaptureBackend::capture`].
    pub id: TargetId,
    /// Monitor or window.
    pub kind: TargetKind,
    /// Monitor name or window title, for menus and screen readers.
    pub name: String,
    /// Bounds in physical desktop pixels.
    pub bounds: PhysicalRect,
    /// Physical pixels per logical pixel (1.0, 1.25, 2.0, ...).
    pub scale_factor: f32,
    /// True for the primary monitor.
    pub primary: bool,
}

/// One captured image and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The pixels.
    pub image: Image,
    /// The target captured.
    pub target: TargetId,
    /// Area captured, in physical desktop pixels.
    pub bounds: PhysicalRect,
    /// Scale factor of the source monitor.
    pub scale_factor: f32,
}

/// Why capture failed. Messages are written for end users.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// This session or platform cannot be captured by the chosen backend.
    #[error("{0}")]
    Unsupported(String),
    /// The OS denied permission (for example macOS Screen Recording).
    #[error("screen capture permission was denied: {0}")]
    PermissionDenied(String),
    /// The target no longer exists (window closed, monitor unplugged).
    #[error("capture target {0} was not found; it may have closed or been unplugged")]
    NotFound(TargetId),
    /// The requested region does not overlap the target.
    #[error("the requested region is outside the target")]
    EmptyRegion,
    /// The platform API failed.
    #[error("capture failed: {0}")]
    Backend(String),
}

/// Shorthand result type.
pub type Result<T, E = CaptureError> = std::result::Result<T, E>;

/// A screen capture implementation.
pub trait CaptureBackend: Send {
    /// Short name, for logs and `--json` output.
    fn name(&self) -> &'static str;

    /// Monitors (and windows, where supported), primary monitor first.
    ///
    /// # Errors
    /// When the platform cannot enumerate targets.
    fn targets(&mut self) -> Result<Vec<Target>>;

    /// Captures `target`, optionally limited to `region` (physical desktop pixels).
    ///
    /// # Errors
    /// See [`CaptureError`].
    fn capture(&mut self, target: &TargetId, region: Option<PhysicalRect>) -> Result<Frame>;
}

/// Which backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendChoice {
    /// The best native backend for this session.
    #[default]
    Auto,
    /// The deterministic test pattern.
    Synthetic,
}

/// Opens a backend.
///
/// # Errors
/// [`CaptureError::Unsupported`] when this session has no usable native backend (for example a
/// pure Wayland session, until the portal backend lands).
pub fn detect(choice: BackendChoice) -> Result<Box<dyn CaptureBackend>> {
    match choice {
        BackendChoice::Synthetic => Ok(Box::new(SyntheticBackend::new())),
        BackendChoice::Auto => native(),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn native() -> Result<Box<dyn CaptureBackend>> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    if wayland && !x11 {
        return Err(CaptureError::Unsupported(
            "this is a Wayland session; capture there needs the xdg-desktop-portal backend, which \
             is not implemented yet. Log into an X11 session or use `--backend synthetic` to try \
             the rest of Tellshot."
                .into(),
        ));
    }
    Ok(Box::new(x11::X11Backend::connect(wayland)?))
}

#[cfg(any(windows, target_os = "macos"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "same signature on every platform; other backends can fail"
)]
fn native() -> Result<Box<dyn CaptureBackend>> {
    Ok(Box::new(xcap_backend::XcapBackend::new()))
}

#[cfg(not(any(unix, windows)))]
fn native() -> Result<Box<dyn CaptureBackend>> {
    Err(CaptureError::Unsupported("screen capture is not available on this platform".into()))
}

/// Crops `image`, whose top-left corner sits at `origin`, to `region` (both in desktop pixels).
pub(crate) fn crop_to_region(
    image: Image,
    origin: PhysicalRect,
    region: Option<PhysicalRect>,
) -> Result<(Image, PhysicalRect)> {
    let Some(region) = region else {
        return Ok((image, origin));
    };
    let area = region.intersection(origin).ok_or(CaptureError::EmptyRegion)?;
    let local = tellshot_imaging::PixelRect {
        x: u32::try_from(i64::from(area.x) - i64::from(origin.x))
            .map_err(|_| CaptureError::EmptyRegion)?,
        y: u32::try_from(i64::from(area.y) - i64::from(origin.y))
            .map_err(|_| CaptureError::EmptyRegion)?,
        width: area.width,
        height: area.height,
    };
    let cropped = image.crop(local).ok_or(CaptureError::EmptyRegion)?;
    Ok((cropped, area))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_handles_negative_origins() {
        let left = PhysicalRect { x: -1920, y: 0, width: 1920, height: 1080 };
        let region = PhysicalRect { x: -100, y: 50, width: 300, height: 100 };
        assert_eq!(
            left.intersection(region),
            Some(PhysicalRect { x: -100, y: 50, width: 100, height: 100 }),
            "clipped at the monitor's right edge"
        );
        let right = PhysicalRect { x: 0, y: 0, width: 10, height: 10 };
        assert_eq!(left.intersection(right), None, "touching edges do not overlap");
    }
}
