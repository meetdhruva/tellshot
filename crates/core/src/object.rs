//! Annotation objects: what is drawn on top of the base image.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::geometry::valid_scalar;
use crate::{Color, Limits, Point, Rect, palette};

/// Stable identifier of an object within one document. Never reused, even after deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct ObjectId(pub u64);

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// The geometry and kind of an annotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    /// A straight arrow pointing at `to`.
    Arrow {
        /// Tail.
        from: Point,
        /// Head.
        to: Point,
    },
    /// A straight line.
    Line {
        /// Start.
        from: Point,
        /// End.
        to: Point,
    },
    /// A rectangle outline, filled when the style has a fill.
    Rectangle {
        /// Bounds.
        rect: Rect,
        /// Corner radius in document pixels.
        #[serde(default)]
        corner_radius: f32,
    },
    /// An ellipse inscribed in `rect`.
    Ellipse {
        /// Bounds.
        rect: Rect,
    },
    /// A text label whose first baseline starts at `origin`.
    Text {
        /// Top-left anchor.
        origin: Point,
        /// The text; may contain newlines.
        text: String,
        /// Font size in document pixels.
        font_size: f32,
    },
    /// A numbered step marker. Numbers follow z-order and update automatically.
    Step {
        /// Centre of the marker.
        center: Point,
        /// Radius of the marker.
        radius: f32,
    },
    /// A translucent highlighter band.
    Highlight {
        /// Bounds.
        rect: Rect,
    },
    /// Blurs the base image inside `rect`. Not a privacy tool: use [`Shape::Redact`].
    Blur {
        /// Area to blur.
        rect: Rect,
        /// Blur radius in document pixels.
        radius: f32,
    },
    /// Pixelates the base image inside `rect`. Not a privacy tool: use [`Shape::Redact`].
    Pixelate {
        /// Area to pixelate.
        rect: Rect,
        /// Size of each block in document pixels.
        block_size: f32,
    },
    /// Covers `rect` with opaque black. Export overwrites the underlying pixels, so redacted
    /// content cannot be recovered from the exported file.
    Redact {
        /// Area to redact.
        rect: Rect,
    },
    /// A freehand pen stroke.
    Freehand {
        /// The stroke, in drawing order.
        points: Vec<Point>,
    },
}

/// The kind of a [`Shape`], without its data. Used for tools, labels and styling defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[expect(missing_docs, reason = "variants mirror Shape, which documents each one")]
pub enum ShapeKind {
    Arrow,
    Line,
    Rectangle,
    Ellipse,
    Text,
    Step,
    Highlight,
    Blur,
    Pixelate,
    Redact,
    Freehand,
}

impl ShapeKind {
    /// Every kind, in toolbar order.
    pub const ALL: [Self; 11] = [
        Self::Arrow,
        Self::Line,
        Self::Rectangle,
        Self::Ellipse,
        Self::Text,
        Self::Step,
        Self::Highlight,
        Self::Blur,
        Self::Pixelate,
        Self::Redact,
        Self::Freehand,
    ];

    /// A human-readable, lower-case name for labels, undo menus and screen readers.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Arrow => "arrow",
            Self::Line => "line",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Text => "text",
            Self::Step => "step marker",
            Self::Highlight => "highlight",
            Self::Blur => "blur",
            Self::Pixelate => "pixelate",
            Self::Redact => "redaction",
            Self::Freehand => "pen stroke",
        }
    }

    /// The style a new object of this kind gets when the caller does not supply one.
    #[must_use]
    pub const fn default_style(self) -> Style {
        match self {
            Self::Highlight => Style {
                stroke: palette::YELLOW,
                fill: Some(palette::YELLOW.with_alpha(0x66)),
                stroke_width: 0.0,
            },
            Self::Redact => {
                Style { stroke: palette::BLACK, fill: Some(palette::BLACK), stroke_width: 0.0 }
            }
            Self::Step => {
                Style { stroke: palette::WHITE, fill: Some(palette::VERMILION), stroke_width: 2.0 }
            }
            Self::Text => Style { stroke: palette::VERMILION, fill: None, stroke_width: 0.0 },
            Self::Blur | Self::Pixelate => {
                Style { stroke: palette::BLACK, fill: None, stroke_width: 0.0 }
            }
            Self::Arrow | Self::Line | Self::Rectangle | Self::Ellipse | Self::Freehand => {
                Style { stroke: palette::VERMILION, fill: None, stroke_width: 4.0 }
            }
        }
    }

    /// True for kinds that change base-image pixels rather than drawing on top of them.
    #[must_use]
    pub const fn is_raster_effect(self) -> bool {
        matches!(self, Self::Blur | Self::Pixelate)
    }
}

impl Shape {
    /// The kind of this shape.
    #[must_use]
    pub const fn kind(&self) -> ShapeKind {
        match self {
            Self::Arrow { .. } => ShapeKind::Arrow,
            Self::Line { .. } => ShapeKind::Line,
            Self::Rectangle { .. } => ShapeKind::Rectangle,
            Self::Ellipse { .. } => ShapeKind::Ellipse,
            Self::Text { .. } => ShapeKind::Text,
            Self::Step { .. } => ShapeKind::Step,
            Self::Highlight { .. } => ShapeKind::Highlight,
            Self::Blur { .. } => ShapeKind::Blur,
            Self::Pixelate { .. } => ShapeKind::Pixelate,
            Self::Redact { .. } => ShapeKind::Redact,
            Self::Freehand { .. } => ShapeKind::Freehand,
        }
    }

    /// Approximate bounding box (text uses an estimate; the renderer knows exact metrics).
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Arrow { from, to } | Self::Line { from, to } => Rect::from_points(*from, *to),
            Self::Rectangle { rect, .. }
            | Self::Ellipse { rect }
            | Self::Highlight { rect }
            | Self::Blur { rect, .. }
            | Self::Pixelate { rect, .. }
            | Self::Redact { rect } => *rect,
            Self::Text { origin, text, font_size } => {
                let lines = text.lines().count().max(1);
                let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
                Rect::new(
                    origin.x,
                    origin.y,
                    approx(longest) * font_size * 0.6,
                    approx(lines) * font_size * 1.25,
                )
            }
            Self::Step { center, radius } => {
                Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0)
            }
            Self::Freehand { points } => Rect::bounding(points.iter().copied()).unwrap_or_default(),
        }
    }

    /// Returns this shape moved by `(dx, dy)`.
    #[must_use]
    pub fn translated(&self, dx: f32, dy: f32) -> Self {
        let mut out = self.clone();
        match &mut out {
            Self::Arrow { from, to } | Self::Line { from, to } => {
                *from = from.offset(dx, dy);
                *to = to.offset(dx, dy);
            }
            Self::Rectangle { rect, .. }
            | Self::Ellipse { rect }
            | Self::Highlight { rect }
            | Self::Blur { rect, .. }
            | Self::Pixelate { rect, .. }
            | Self::Redact { rect } => *rect = rect.offset(dx, dy),
            Self::Text { origin, .. } => *origin = origin.offset(dx, dy),
            Self::Step { center, .. } => *center = center.offset(dx, dy),
            Self::Freehand { points } => {
                for p in points {
                    *p = p.offset(dx, dy);
                }
            }
        }
        out
    }

    /// True when `p` is within `tolerance` of the visible part of the shape.
    #[must_use]
    pub fn hit(&self, p: Point, tolerance: f32) -> bool {
        match self {
            Self::Arrow { from, to } | Self::Line { from, to } => {
                p.distance_to_segment(*from, *to) <= tolerance
            }
            Self::Freehand { points } => match points.as_slice() {
                [] => false,
                [only] => p.distance(*only) <= tolerance,
                many => many.windows(2).any(|w| match w {
                    [a, b] => p.distance_to_segment(*a, *b) <= tolerance,
                    _ => false,
                }),
            },
            Self::Step { center, radius } => p.distance(*center) <= radius + tolerance,
            Self::Ellipse { rect } => {
                let r = rect.inflate(tolerance);
                let (rx, ry) = (r.width / 2.0, r.height / 2.0);
                if rx <= 0.0 || ry <= 0.0 {
                    return false;
                }
                let c = r.center();
                let (nx, ny) = ((p.x - c.x) / rx, (p.y - c.y) / ry);
                libm::fmaf(nx, nx, ny * ny) <= 1.0
            }
            Self::Rectangle { .. }
            | Self::Text { .. }
            | Self::Highlight { .. }
            | Self::Blur { .. }
            | Self::Pixelate { .. }
            | Self::Redact { .. } => self.bounds().inflate(tolerance).contains(p),
        }
    }

    /// Checks coordinates, sizes and lengths against [`Limits`].
    ///
    /// # Errors
    /// Returns a short description of the first problem found.
    pub fn validate(&self) -> Result<(), &'static str> {
        let ok = match self {
            Self::Arrow { from, to } | Self::Line { from, to } => from.is_valid() && to.is_valid(),
            Self::Rectangle { rect, corner_radius } => {
                rect.is_valid() && valid_scalar(*corner_radius)
            }
            Self::Ellipse { rect } | Self::Highlight { rect } | Self::Redact { rect } => {
                rect.is_valid()
            }
            Self::Blur { rect, radius } => rect.is_valid() && valid_scalar(*radius),
            Self::Pixelate { rect, block_size } => {
                rect.is_valid() && valid_scalar(*block_size) && *block_size >= 1.0
            }
            Self::Step { center, radius } => center.is_valid() && valid_scalar(*radius),
            Self::Text { origin, text, font_size } => {
                if text.chars().count() > Limits::MAX_TEXT_CHARS {
                    return Err("text is too long");
                }
                origin.is_valid() && valid_scalar(*font_size) && *font_size > 0.0
            }
            Self::Freehand { points } => {
                if points.len() > Limits::MAX_FREEHAND_POINTS {
                    return Err("freehand stroke has too many points");
                }
                points.iter().all(|p| p.is_valid())
            }
        };
        if ok { Ok(()) } else { Err("coordinates or sizes are out of range") }
    }
}

/// How an object is painted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Style {
    /// Outline, line or text colour.
    pub stroke: Color,
    /// Interior colour, if filled.
    #[serde(default)]
    pub fill: Option<Color>,
    /// Outline or line width in document pixels.
    pub stroke_width: f32,
}

impl Style {
    /// Checks sizes against [`Limits`].
    ///
    /// # Errors
    /// Returns a short description when the stroke width is out of range.
    pub fn validate(&self) -> Result<(), &'static str> {
        if valid_scalar(self.stroke_width) { Ok(()) } else { Err("stroke width is out of range") }
    }
}

/// One annotation in a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Object {
    /// Stable identifier.
    pub id: ObjectId,
    /// Geometry.
    pub shape: Shape,
    /// Paint.
    pub style: Style,
}

impl Object {
    /// A one-line description for screen readers, undo labels and AI agents.
    ///
    /// `step` is the marker's number when the object is a step marker.
    #[must_use]
    pub fn describe(&self, step: Option<u32>) -> String {
        let r = |v: f32| libm::roundf(v);
        match &self.shape {
            Shape::Arrow { from, to } => format!(
                "Arrow {} from ({}, {}) to ({}, {})",
                self.id,
                r(from.x),
                r(from.y),
                r(to.x),
                r(to.y)
            ),
            Shape::Line { from, to } => format!(
                "Line {} from ({}, {}) to ({}, {})",
                self.id,
                r(from.x),
                r(from.y),
                r(to.x),
                r(to.y)
            ),
            Shape::Text { text, .. } => {
                let preview: String = text.chars().take(60).collect();
                let ellipsis = if text.chars().count() > 60 { "…" } else { "" };
                format!("Text {}: \"{preview}{ellipsis}\"", self.id)
            }
            Shape::Step { center, .. } => format!(
                "Step {} {} at ({}, {})",
                step.unwrap_or(0),
                self.id,
                r(center.x),
                r(center.y)
            ),
            Shape::Freehand { points } => {
                format!("Pen stroke {} with {} points", self.id, points.len())
            }
            other @ (Shape::Rectangle { .. }
            | Shape::Ellipse { .. }
            | Shape::Highlight { .. }
            | Shape::Blur { .. }
            | Shape::Pixelate { .. }
            | Shape::Redact { .. }) => {
                let b = other.bounds();
                let label = other.kind().label();
                let mut chars = label.chars();
                let capitalised: String =
                    chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars).collect();
                format!(
                    "{capitalised} {} at ({}, {}), {} by {}",
                    self.id,
                    r(b.x),
                    r(b.y),
                    r(b.width),
                    r(b.height)
                )
            }
        }
    }
}

/// `usize` → `f32` for small counts used in layout estimates.
fn approx(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(shape: Shape) -> Object {
        let style = shape.kind().default_style();
        Object { id: ObjectId(7), shape, style }
    }

    #[test]
    fn hit_testing_matches_what_users_expect() {
        let arrow = Shape::Arrow { from: Point::new(0.0, 0.0), to: Point::new(100.0, 0.0) };
        assert!(arrow.hit(Point::new(50.0, 3.0), 4.0), "near the shaft");
        assert!(!arrow.hit(Point::new(50.0, 10.0), 4.0), "far from the shaft");

        let ellipse = Shape::Ellipse { rect: Rect::new(0.0, 0.0, 100.0, 50.0) };
        assert!(ellipse.hit(Point::new(50.0, 25.0), 0.0), "centre");
        assert!(!ellipse.hit(Point::new(2.0, 2.0), 0.0), "corner outside the curve");

        let rect = Shape::Rectangle { rect: Rect::new(10.0, 10.0, 10.0, 10.0), corner_radius: 0.0 };
        assert!(rect.hit(Point::new(15.0, 15.0), 0.0), "inside an unfilled rectangle selects it");
    }

    #[test]
    fn translate_moves_every_point() {
        let pen =
            Shape::Freehand { points: alloc::vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)] };
        let moved = pen.translated(10.0, 5.0);
        assert_eq!(
            moved,
            Shape::Freehand { points: alloc::vec![Point::new(10.0, 5.0), Point::new(11.0, 6.0)] },
            "freehand points"
        );
    }

    #[test]
    fn validation_rejects_hostile_input() {
        let nan = Shape::Arrow { from: Point::new(f32::NAN, 0.0), to: Point::new(0.0, 0.0) };
        assert!(nan.validate().is_err(), "NaN coordinates");
        let huge = Shape::Text {
            origin: Point::new(0.0, 0.0),
            text: "x".repeat(Limits::MAX_TEXT_CHARS + 1),
            font_size: 12.0,
        };
        assert!(huge.validate().is_err(), "oversized text");
        let zero_block = Shape::Pixelate { rect: Rect::new(0.0, 0.0, 9.0, 9.0), block_size: 0.0 };
        assert!(zero_block.validate().is_err(), "zero pixelate block would divide by zero");
    }

    #[test]
    fn descriptions_are_readable() {
        let a = object(Shape::Arrow { from: Point::new(1.4, 2.0), to: Point::new(30.0, 40.6) });
        assert_eq!(a.describe(None), "Arrow #7 from (1, 2) to (30, 41)", "arrow");
        let r = object(Shape::Redact { rect: Rect::new(0.0, 0.0, 10.0, 5.0) });
        assert_eq!(r.describe(None), "Redaction #7 at (0, 0), 10 by 5", "redaction");
    }

    #[test]
    fn json_shape_is_agent_friendly() {
        let s = Shape::Step { center: Point::new(5.0, 6.0), radius: 14.0 };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"kind":"step","center":{"x":5.0,"y":6.0},"radius":14.0}"#, "format");
        assert!(
            serde_json::from_str::<Shape>(
                r#"{"kind":"step","center":{"x":1,"y":1},"radius":2,"oops":1}"#
            )
            .is_err(),
            "unknown fields are rejected, not ignored"
        );
    }
}
