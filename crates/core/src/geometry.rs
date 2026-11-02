//! Document-space geometry. One unit is one pixel of the base image.

use serde::{Deserialize, Serialize};

use crate::Limits;

/// A point in document space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Point {
    /// Horizontal position; grows to the right.
    pub x: f32,
    /// Vertical position; grows downward.
    pub y: f32,
}

impl Point {
    /// Creates a point.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns this point moved by `(dx, dy)`.
    #[must_use]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Euclidean distance to `other`.
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        libm::hypotf(other.x - self.x, other.y - self.y)
    }

    /// Shortest distance from this point to the segment `a`–`b`.
    #[must_use]
    pub fn distance_to_segment(self, a: Self, b: Self) -> f32 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = libm::fmaf(dx, dx, dy * dy);
        if len_sq <= f32::EPSILON {
            return self.distance(a);
        }
        let t = libm::fmaf(self.x - a.x, dx, (self.y - a.y) * dy) / len_sq;
        let t = t.clamp(0.0, 1.0);
        self.distance(Self::new(libm::fmaf(t, dx, a.x), libm::fmaf(t, dy, a.y)))
    }

    /// True when both coordinates are finite and within [`Limits::MAX_COORDINATE`].
    #[must_use]
    pub fn is_valid(self) -> bool {
        valid_coordinate(self.x) && valid_coordinate(self.y)
    }
}

/// A width and height in document space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Size {
    /// Horizontal extent.
    pub width: f32,
    /// Vertical extent.
    pub height: f32,
}

/// An axis-aligned rectangle in document space. Width and height are never negative.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Horizontal extent (non-negative).
    pub width: f32,
    /// Vertical extent (non-negative).
    pub height: f32,
}

impl Rect {
    /// Creates a rectangle, normalising negative sizes so width and height are non-negative.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::from_points(Point::new(x, y), Point::new(x + width, y + height))
    }

    /// The rectangle spanned by two opposite corners, in any order (for example a drag gesture).
    #[must_use]
    pub fn from_points(a: Point, b: Point) -> Self {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        Self { x: x0, y: y0, width: x1 - x0, height: y1 - y0 }
    }

    /// The smallest rectangle containing every point, or `None` for an empty iterator.
    #[must_use]
    pub fn bounding(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
        for p in points {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Some(Self { x: x0, y: y0, width: x1 - x0, height: y1 - y0 })
    }

    /// Right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Centre point.
    #[must_use]
    pub fn center(self) -> Point {
        Point::new(libm::fmaf(self.width, 0.5, self.x), libm::fmaf(self.height, 0.5, self.y))
    }

    /// True when `p` lies inside or on the edge.
    #[must_use]
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// Returns this rectangle grown by `amount` on every side (shrunk when negative).
    #[must_use]
    pub fn inflate(self, amount: f32) -> Self {
        Self::new(
            self.x - amount,
            self.y - amount,
            libm::fmaf(amount, 2.0, self.width).max(0.0),
            libm::fmaf(amount, 2.0, self.height).max(0.0),
        )
    }

    /// Returns this rectangle moved by `(dx, dy)`.
    #[must_use]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self { x: self.x + dx, y: self.y + dy, ..self }
    }

    /// The overlapping area of two rectangles, or `None` when they do not overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (x1 > x0 && y1 > y0).then_some(Self { x: x0, y: y0, width: x1 - x0, height: y1 - y0 })
    }

    /// The smallest rectangle containing both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        Self { x: x0, y: y0, width: x1 - x0, height: y1 - y0 }
    }

    /// True when the area is zero.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// True when every field is finite, sizes are non-negative and all edges are within limits.
    #[must_use]
    pub fn is_valid(self) -> bool {
        valid_coordinate(self.x)
            && valid_coordinate(self.y)
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height >= 0.0
            && valid_coordinate(self.right())
            && valid_coordinate(self.bottom())
    }
}

/// True when `v` is finite and within [`Limits::MAX_COORDINATE`].
pub(crate) fn valid_coordinate(v: f32) -> bool {
    v.is_finite() && v.abs() <= Limits::MAX_COORDINATE
}

/// True when `v` is finite and within `0..=Limits::MAX_SCALAR`.
pub(crate) fn valid_scalar(v: f32) -> bool {
    v.is_finite() && (0.0..=Limits::MAX_SCALAR).contains(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_points_normalises_any_drag_direction() {
        let r = Rect::from_points(Point::new(10.0, 20.0), Point::new(4.0, 2.0));
        assert_eq!(r, Rect { x: 4.0, y: 2.0, width: 6.0, height: 18.0 }, "drag up-left");
        assert_eq!(
            Rect::new(10.0, 10.0, -5.0, -5.0),
            Rect::new(5.0, 5.0, 5.0, 5.0),
            "negative size"
        );
    }

    #[test]
    fn segment_distance_handles_endpoints_and_degenerate_segments() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert!((Point::new(5.0, 3.0).distance_to_segment(a, b) - 3.0).abs() < 1e-5, "middle");
        assert!((Point::new(13.0, 4.0).distance_to_segment(a, b) - 5.0).abs() < 1e-5, "past end");
        assert!((Point::new(3.0, 4.0).distance_to_segment(a, a) - 5.0).abs() < 1e-5, "zero length");
    }

    #[test]
    fn intersection_and_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersection(b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)), "overlap");
        assert_eq!(a.intersection(Rect::new(20.0, 20.0, 1.0, 1.0)), None, "disjoint");
        assert_eq!(a.union(b), Rect::new(0.0, 0.0, 15.0, 15.0), "union");
    }

    #[test]
    fn validity_rejects_nan_infinity_and_huge_values() {
        assert!(!Point::new(f32::NAN, 0.0).is_valid(), "NaN");
        assert!(!Point::new(f32::INFINITY, 0.0).is_valid(), "infinity");
        assert!(!Point::new(1e9, 0.0).is_valid(), "beyond limit");
        assert!(!Rect { x: 0.0, y: 0.0, width: -1.0, height: 1.0 }.is_valid(), "negative width");
        assert!(Rect::new(0.0, 0.0, 100.0, 100.0).is_valid(), "ordinary rect");
    }
}
