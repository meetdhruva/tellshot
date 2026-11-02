//! A deterministic fake desktop with two monitors. Lets CI, tests and AI agents exercise the
//! whole capture → annotate → export flow without a display server or permissions.

use tellshot_imaging::Image;

use crate::{
    CaptureBackend, CaptureError, Frame, PhysicalRect, Result, Target, TargetId, TargetKind,
    crop_to_region,
};

/// The synthetic backend: a 1280×720 primary monitor and an 800×600 monitor to its left.
#[derive(Debug, Clone, Default)]
pub struct SyntheticBackend {
    _private: (),
}

impl SyntheticBackend {
    /// Creates the backend.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    fn monitors() -> [Target; 2] {
        [
            Target {
                id: TargetId("synthetic:primary".into()),
                kind: TargetKind::Monitor,
                name: "Synthetic primary (1280×720)".into(),
                bounds: PhysicalRect { x: 0, y: 0, width: 1280, height: 720 },
                scale_factor: 1.0,
                primary: true,
            },
            Target {
                id: TargetId("synthetic:left".into()),
                kind: TargetKind::Monitor,
                name: "Synthetic left (800×600, 2× scale)".into(),
                bounds: PhysicalRect { x: -800, y: 0, width: 800, height: 600 },
                scale_factor: 2.0,
                primary: false,
            },
        ]
    }
}

impl CaptureBackend for SyntheticBackend {
    fn name(&self) -> &'static str {
        "synthetic"
    }

    fn targets(&mut self) -> Result<Vec<Target>> {
        Ok(Self::monitors().to_vec())
    }

    fn capture(&mut self, target: &TargetId, region: Option<PhysicalRect>) -> Result<Frame> {
        let monitor = Self::monitors()
            .into_iter()
            .find(|m| &m.id == target)
            .ok_or_else(|| CaptureError::NotFound(target.clone()))?;
        let image = pattern(monitor.bounds.width, monitor.bounds.height)?;
        let (image, bounds) = crop_to_region(image, monitor.bounds, region)?;
        Ok(Frame { image, target: monitor.id, bounds, scale_factor: monitor.scale_factor })
    }
}

/// A recognisable pattern: a soft gradient, a title bar, and dark "text" bars, so captures,
/// crops and annotations are easy to eyeball.
fn pattern(width: u32, height: u32) -> Result<Image> {
    let mut data = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let px = if y < 32 {
                [32, 36, 44, 255]
            } else if y % 28 < 8 && x % 220 > 24 && x % 220 < 190 {
                [52, 58, 70, 255]
            } else {
                let gx = u8::try_from(x * 40 / width.max(1)).unwrap_or(0);
                let gy = u8::try_from(y * 30 / height.max(1)).unwrap_or(0);
                [215 + gx, 222 + gy, 235, 255]
            };
            data.extend_from_slice(&px);
        }
    }
    Image::from_rgba(width, height, data).map_err(|e| CaptureError::Backend(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_whole_monitors_and_regions() {
        let mut b = SyntheticBackend::new();
        let targets = b.targets().unwrap();
        assert!(targets.first().is_some_and(|t| t.primary), "primary first");
        let frame = b.capture(&TargetId("synthetic:left".into()), None).unwrap();
        assert_eq!((frame.image.width(), frame.image.height()), (800, 600), "whole monitor");

        let region = PhysicalRect { x: -100, y: 10, width: 400, height: 50 };
        let part = b.capture(&TargetId("synthetic:left".into()), Some(region)).unwrap();
        assert_eq!((part.image.width(), part.image.height()), (100, 50), "clipped to the monitor");
        assert_eq!(part.bounds, PhysicalRect { x: -100, y: 10, width: 100, height: 50 }, "bounds");
    }

    #[test]
    fn unknown_targets_and_empty_regions_are_errors() {
        let mut b = SyntheticBackend::new();
        assert!(
            matches!(b.capture(&TargetId("nope".into()), None), Err(CaptureError::NotFound(_))),
            "unknown"
        );
        let outside = PhysicalRect { x: 5000, y: 5000, width: 10, height: 10 };
        assert!(
            matches!(
                b.capture(&TargetId("synthetic:primary".into()), Some(outside)),
                Err(CaptureError::EmptyRegion)
            ),
            "outside"
        );
    }
}
