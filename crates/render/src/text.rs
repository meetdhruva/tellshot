//! Glyph outlines for text objects and step numbers.
//!
//! Deliberately minimal for v0: one sans-serif face, left-to-right, no shaping or kerning.
//! Complex scripts and font fallback arrive with a proper shaper (parley); see ADR 0003.

use std::sync::Arc;

use tiny_skia::{Path, PathBuilder};

/// Loaded font data used for text. Cheap to clone.
#[derive(Clone)]
pub struct Fonts {
    face: Option<Arc<FaceData>>,
}

struct FaceData {
    data: Vec<u8>,
    index: u32,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fonts").field("loaded", &self.face.is_some()).finish()
    }
}

/// A laid-out line of text: one path per glyph plus the total advance.
pub(crate) struct Laid {
    pub(crate) path: Option<Path>,
    pub(crate) width: f32,
}

impl Fonts {
    /// No fonts: text objects and step numbers are not drawn. Useful for headless tests.
    #[must_use]
    pub const fn none() -> Self {
        Self { face: None }
    }

    /// Finds a sans-serif system font. Scans font directories, so call once and reuse.
    #[must_use]
    pub fn system() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let query = fontdb::Query {
            families: &[
                fontdb::Family::Name("Inter"),
                fontdb::Family::Name("Segoe UI"),
                fontdb::Family::Name("Helvetica Neue"),
                fontdb::Family::Name("DejaVu Sans"),
                fontdb::Family::SansSerif,
            ],
            weight: fontdb::Weight::SEMIBOLD,
            ..fontdb::Query::default()
        };
        let face = db.query(&query).and_then(|id| {
            db.with_face_data(id, |data, index| FaceData { data: data.to_vec(), index })
        });
        Self { face: face.map(Arc::new) }
    }

    /// Uses the given font file bytes (TTF/OTF). Returns [`Fonts::none`] if they do not parse.
    #[must_use]
    pub fn from_bytes(data: Vec<u8>) -> Self {
        if ttf_parser::Face::parse(&data, 0).is_err() {
            return Self::none();
        }
        Self { face: Some(Arc::new(FaceData { data, index: 0 })) }
    }

    /// True when a font is available.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.face.is_some()
    }

    /// Lays out `text` with its top-left corner at `(x, y)`; lines are separated by `\n`.
    pub(crate) fn layout(&self, text: &str, x: f32, y: f32, size: f32) -> Laid {
        let Some(face_data) = &self.face else {
            return Laid { path: None, width: 0.0 };
        };
        let Ok(face) = ttf_parser::Face::parse(&face_data.data, face_data.index) else {
            return Laid { path: None, width: 0.0 };
        };
        let scale = size / f32::from(face.units_per_em());
        let ascent = f32::from(face.ascender()) * scale;
        let line_height = f32::from(face.ascender() - face.descender() + face.line_gap()) * scale;
        let mut builder = PathBuilder::new();
        let mut widest = 0.0_f32;
        let mut baseline = y + ascent;
        for line in text.split('\n') {
            let mut pen = x;
            for ch in line.chars() {
                let Some(glyph) = face.glyph_index(ch) else {
                    pen = size.mul_add(0.5, pen);
                    continue;
                };
                let mut outline = Outline { builder: &mut builder, scale, dx: pen, dy: baseline };
                let _bbox = face.outline_glyph(glyph, &mut outline);
                pen += face.glyph_hor_advance(glyph).map_or(size * 0.5, |a| f32::from(a) * scale);
            }
            widest = widest.max(pen - x);
            baseline += line_height;
        }
        Laid { path: builder.finish(), width: widest }
    }
}

/// Bridges ttf-parser outlines (y up, font units) to tiny-skia paths (y down, pixels).
struct Outline<'a> {
    builder: &'a mut PathBuilder,
    scale: f32,
    dx: f32,
    dy: f32,
}

impl Outline<'_> {
    fn x(&self, v: f32) -> f32 {
        v.mul_add(self.scale, self.dx)
    }
    fn y(&self, v: f32) -> f32 {
        (-v).mul_add(self.scale, self.dy)
    }
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = (self.x(x), self.y(y));
        self.builder.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = (self.x(x), self.y(y));
        self.builder.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1, x, y) = (self.x(x1), self.y(y1), self.x(x), self.y(y));
        self.builder.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1, x2, y2) = (self.x(x1), self.y(y1), self.x(x2), self.y(y2));
        let (x, y) = (self.x(x), self.y(y));
        self.builder.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.builder.close();
    }
}
