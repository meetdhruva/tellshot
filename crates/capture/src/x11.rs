//! X11 capture via `x11rb` (pure Rust, no libxcb needed at build time).
//!
//! Monitors come from RandR 1.5 `GetMonitors`; pixels from `GetImage` on the root window.
//! MIT-SHM (zero-copy) and window capture are on the roadmap.

use x11rb::connection::Connection as _;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, ImageOrder, Window};
use x11rb::rust_connection::RustConnection;

use tellshot_imaging::Image;

use crate::{
    CaptureBackend, CaptureError, Frame, PhysicalRect, Result, Target, TargetId, TargetKind,
};

/// Captures from an X11 display (including XWayland, where native Wayland windows show black).
#[derive(Debug)]
pub(crate) struct X11Backend {
    conn: RustConnection,
    root: Window,
    root_size: (u16, u16),
    masks: (u32, u32, u32),
    bits_per_pixel: u8,
    lsb_first: bool,
    xwayland: bool,
}

fn backend_err(e: impl std::fmt::Display) -> CaptureError {
    CaptureError::Backend(e.to_string())
}

impl X11Backend {
    pub(crate) fn connect(under_wayland: bool) -> Result<Self> {
        let (conn, screen_num) = x11rb::connect(None).map_err(|e| {
            CaptureError::Unsupported(format!("could not connect to the X display: {e}"))
        })?;
        let setup = conn.setup();
        let screen = setup.roots.get(screen_num).ok_or_else(|| backend_err("no X screen"))?;
        let depth = screen.root_depth;
        let visual = screen
            .allowed_depths
            .iter()
            .flat_map(|d| &d.visuals)
            .find(|v| v.visual_id == screen.root_visual)
            .ok_or_else(|| backend_err("root visual not found"))?;
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == depth)
            .ok_or_else(|| backend_err("no pixmap format for the root depth"))?;
        if format.bits_per_pixel != 32 {
            return Err(CaptureError::Unsupported(format!(
                "{}-bit X displays are not supported yet",
                format.bits_per_pixel
            )));
        }
        Ok(Self {
            root: screen.root,
            root_size: (screen.width_in_pixels, screen.height_in_pixels),
            masks: (visual.red_mask, visual.green_mask, visual.blue_mask),
            bits_per_pixel: format.bits_per_pixel,
            lsb_first: setup.image_byte_order == ImageOrder::LSB_FIRST,
            // Under XWayland only X11 clients are visible; native Wayland windows capture black.
            xwayland: under_wayland,
            conn,
        })
    }

    fn monitors(&self) -> Result<Vec<Target>> {
        let reply = self.conn.randr_get_monitors(self.root, true).map_err(backend_err)?.reply();
        let mut targets = match reply {
            Ok(r) => r
                .monitors
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let name = self
                        .conn
                        .get_atom_name(m.name)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        .map_or_else(
                            || format!("Monitor {}", i + 1),
                            |r| String::from_utf8_lossy(&r.name).into_owned(),
                        );
                    Target {
                        id: TargetId(format!("x11:{name}")),
                        kind: TargetKind::Monitor,
                        name,
                        bounds: PhysicalRect {
                            x: i32::from(m.x),
                            y: i32::from(m.y),
                            width: u32::from(m.width),
                            height: u32::from(m.height),
                        },
                        scale_factor: 1.0,
                        primary: m.primary,
                    }
                })
                .collect(),
            // No RandR 1.5 (for example a bare Xvfb): treat the whole root window as one monitor.
            Err(_) => Vec::new(),
        };
        if targets.is_empty() {
            targets.push(Target {
                id: TargetId("x11:screen".into()),
                kind: TargetKind::Monitor,
                name: "Screen".into(),
                bounds: PhysicalRect {
                    x: 0,
                    y: 0,
                    width: u32::from(self.root_size.0),
                    height: u32::from(self.root_size.1),
                },
                scale_factor: 1.0,
                primary: true,
            });
        }
        if !targets.iter().any(|t| t.primary)
            && let Some(first) = targets.first_mut()
        {
            first.primary = true;
        }
        targets.sort_by_key(|t| !t.primary);
        Ok(targets)
    }

    fn grab(&self, area: PhysicalRect) -> Result<Image> {
        let coord = |v: i32| i16::try_from(v).map_err(|_| CaptureError::EmptyRegion);
        let extent = |v: u32| u16::try_from(v).map_err(|_| CaptureError::EmptyRegion);
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                coord(area.x)?,
                coord(area.y)?,
                extent(area.width)?,
                extent(area.height)?,
                u32::MAX,
            )
            .map_err(backend_err)?
            .reply()
            .map_err(backend_err)?;
        self.to_rgba(area.width, area.height, &reply.data)
    }

    /// Converts 32-bpp X pixels to RGBA using the visual's channel masks.
    fn to_rgba(&self, width: u32, height: u32, data: &[u8]) -> Result<Image> {
        debug_assert_eq!(self.bits_per_pixel, 32, "checked in connect");
        let (rm, gm, bm) = self.masks;
        let channel = |px: u32, mask: u32| -> u8 {
            if mask == 0 {
                return 0;
            }
            let shift = mask.trailing_zeros();
            let max = mask >> shift;
            let v = (px & mask) >> shift;
            u8::try_from(v * 255 / max.max(1)).unwrap_or(u8::MAX)
        };
        let rgba: Vec<u8> = data
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&b| {
                let px = if self.lsb_first { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
                [channel(px, rm), channel(px, gm), channel(px, bm), 255]
            })
            .collect();
        Image::from_rgba(width, height, rgba).map_err(backend_err)
    }
}

impl CaptureBackend for X11Backend {
    fn name(&self) -> &'static str {
        if self.xwayland { "x11 (XWayland: Wayland-native windows appear black)" } else { "x11" }
    }

    fn targets(&mut self) -> Result<Vec<Target>> {
        self.monitors()
    }

    fn capture(&mut self, target: &TargetId, region: Option<PhysicalRect>) -> Result<Frame> {
        let monitor = self
            .monitors()?
            .into_iter()
            .find(|m| &m.id == target)
            .ok_or_else(|| CaptureError::NotFound(target.clone()))?;
        // Grab only what is needed: a small region is far cheaper than a 4K monitor.
        let area = match region {
            Some(r) => r.intersection(monitor.bounds).ok_or(CaptureError::EmptyRegion)?,
            None => monitor.bounds,
        };
        let image = self.grab(area)?;
        Ok(Frame { image, target: monitor.id, bounds: area, scale_factor: monitor.scale_factor })
    }
}
