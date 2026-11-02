//! Windows and macOS capture through the `xcap` crate.
//!
//! Interim: it uses Windows.Graphics.Capture / GDI and CoreGraphics / ScreenCaptureKit under the
//! hood but copies through `image::RgbaImage`. Native adapters with GPU-resident frames replace
//! it behind the same trait (ADR 0004). Compiled in CI on Windows and macOS; not yet exercised
//! by tests there (capture needs a real display and, on macOS, permission).

use tellshot_imaging::Image;

use crate::{
    CaptureBackend, CaptureError, Frame, PhysicalRect, Result, Target, TargetId, TargetKind,
    crop_to_region,
};

#[derive(Debug, Default)]
pub(crate) struct XcapBackend {
    _private: (),
}

impl XcapBackend {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }
}

fn backend_err(e: impl std::fmt::Display) -> CaptureError {
    let message = e.to_string();
    if message.to_ascii_lowercase().contains("permission") {
        CaptureError::PermissionDenied(message)
    } else {
        CaptureError::Backend(message)
    }
}

fn monitor_target(m: &xcap::Monitor) -> Result<Target> {
    let id = m.id().map_err(backend_err)?;
    let name = m.friendly_name().or_else(|_| m.name()).unwrap_or_else(|_| format!("Monitor {id}"));
    let scale_factor = m.scale_factor().unwrap_or(1.0);
    let logical = PhysicalRect {
        x: m.x().map_err(backend_err)?,
        y: m.y().map_err(backend_err)?,
        width: m.width().map_err(backend_err)?,
        height: m.height().map_err(backend_err)?,
    };
    Ok(Target {
        id: TargetId(format!("monitor:{id}")),
        kind: TargetKind::Monitor,
        name,
        bounds: to_physical(logical, scale_factor),
        scale_factor,
        primary: m.is_primary().unwrap_or(false),
    })
}

/// xcap reports macOS geometry in points (CGDisplayBounds) but captures in pixels; on Windows
/// it already reports pixels. Normalise to physical pixels so regions line up with images.
fn to_physical(r: PhysicalRect, scale: f32) -> PhysicalRect {
    if !cfg!(target_os = "macos") || !(scale.is_finite() && scale > 0.0) {
        return r;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "desktop coordinates are far below 2^24; rounding to the nearest pixel is intended"
    )]
    let scaled = PhysicalRect {
        x: (r.x as f32 * scale).round() as i32,
        y: (r.y as f32 * scale).round() as i32,
        width: (r.width as f32 * scale).round() as u32,
        height: (r.height as f32 * scale).round() as u32,
    };
    scaled
}

impl CaptureBackend for XcapBackend {
    fn name(&self) -> &'static str {
        "xcap"
    }

    fn targets(&mut self) -> Result<Vec<Target>> {
        let mut targets = xcap::Monitor::all()
            .map_err(backend_err)?
            .iter()
            .map(monitor_target)
            .collect::<Result<Vec<_>>>()?;
        targets.sort_by_key(|t| !t.primary);
        Ok(targets)
    }

    fn capture(&mut self, target: &TargetId, region: Option<PhysicalRect>) -> Result<Frame> {
        let monitors = xcap::Monitor::all().map_err(backend_err)?;
        for m in &monitors {
            let t = monitor_target(m)?;
            if &t.id != target {
                continue;
            }
            let rgba = m.capture_image().map_err(backend_err)?;
            let (w, h) = (rgba.width(), rgba.height());
            let image = Image::from_rgba(w, h, rgba.into_raw()).map_err(backend_err)?;
            // xcap returns physical pixels; anchor them at the monitor's desktop origin.
            let origin = PhysicalRect { width: w, height: h, ..t.bounds };
            let (image, bounds) = crop_to_region(image, origin, region)?;
            return Ok(Frame { image, target: t.id, bounds, scale_factor: t.scale_factor });
        }
        Err(CaptureError::NotFound(target.clone()))
    }
}
