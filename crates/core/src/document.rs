//! The non-destructive document: an immutable base image plus ordered annotation objects.

use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{Limits, Object, ObjectId, Point, Rect, ShapeKind};

/// Dimensions of the captured image. The pixels themselves live outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BaseImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl BaseImage {
    /// The full image as a rectangle.
    #[must_use]
    pub fn rect(self) -> Rect {
        Rect::new(0.0, 0.0, u32_to_f32(self.width), u32_to_f32(self.height))
    }
}

/// A capture plus its annotations. Objects are stored in z-order: the last one is on top.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Document {
    base: BaseImage,
    #[serde(default)]
    crop: Option<Rect>,
    #[serde(default)]
    objects: Vec<Object>,
    /// The next id to hand out. Ids are never reused, so references stay unambiguous.
    next_id: u64,
}

/// Why a document failed [`Document::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationError {
    /// The base image is empty or larger than [`Limits`] allow.
    ImageSize,
    /// More objects than [`Limits::MAX_OBJECTS`].
    TooManyObjects,
    /// Two objects share an id, or an id is not below `next_id`.
    BadId(ObjectId),
    /// An object or its style has out-of-range values.
    BadObject(ObjectId, &'static str),
    /// The crop rectangle is invalid or outside the image.
    BadCrop,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageSize => f.write_str("the image is empty or too large"),
            Self::TooManyObjects => {
                write!(f, "the document has more than {} objects", Limits::MAX_OBJECTS)
            }
            Self::BadId(id) => write!(f, "object id {id} is duplicated or out of range"),
            Self::BadObject(id, why) => write!(f, "object {id}: {why}"),
            Self::BadCrop => f.write_str("the crop rectangle is outside the image"),
        }
    }
}

impl core::error::Error for ValidationError {}

impl Document {
    /// An empty document over a base image of the given size.
    #[must_use]
    pub const fn new(base: BaseImage) -> Self {
        Self { base, crop: None, objects: Vec::new(), next_id: 1 }
    }

    /// The base image dimensions.
    #[must_use]
    pub const fn base(&self) -> BaseImage {
        self.base
    }

    /// The crop applied on export, if any.
    #[must_use]
    pub const fn crop(&self) -> Option<Rect> {
        self.crop
    }

    /// The visible area: the crop, or the whole image.
    #[must_use]
    pub fn visible_rect(&self) -> Rect {
        self.crop.unwrap_or_else(|| self.base.rect())
    }

    /// Objects in z-order (bottom first).
    #[must_use]
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// The object with this id.
    #[must_use]
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// The z-index of the object with this id.
    #[must_use]
    pub fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    /// The top-most object under `p`, within `tolerance` document pixels.
    #[must_use]
    pub fn hit_test(&self, p: Point, tolerance: f32) -> Option<ObjectId> {
        self.objects.iter().rev().find(|o| o.shape.hit(p, tolerance)).map(|o| o.id)
    }

    /// The number shown on a step marker: its position among step markers in z-order.
    #[must_use]
    pub fn step_number(&self, id: ObjectId) -> Option<u32> {
        let mut n = 0_u32;
        for o in &self.objects {
            if o.shape.kind() == ShapeKind::Step {
                n = n.saturating_add(1);
                if o.id == id {
                    return Some(n);
                }
            }
        }
        None
    }

    /// Checks every invariant. Call after loading a file or receiving a document from outside.
    ///
    /// # Errors
    /// Returns the first violated invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let BaseImage { width, height } = self.base;
        if width == 0
            || height == 0
            || width > Limits::MAX_IMAGE_DIMENSION
            || height > Limits::MAX_IMAGE_DIMENSION
            || u64::from(width) * u64::from(height) > Limits::MAX_IMAGE_PIXELS
        {
            return Err(ValidationError::ImageSize);
        }
        if self.objects.len() > Limits::MAX_OBJECTS {
            return Err(ValidationError::TooManyObjects);
        }
        // Ids are allocated from next_id; a hostile value would make future ids collide.
        if self.next_id == 0 || self.next_id == u64::MAX {
            return Err(ValidationError::BadId(ObjectId(self.next_id)));
        }
        if let Some(crop) = self.crop {
            let inside = self.base.rect().inflate(0.5);
            if !crop.is_valid()
                || crop.is_empty()
                || crop.x < inside.x
                || crop.y < inside.y
                || crop.right() > inside.right()
                || crop.bottom() > inside.bottom()
            {
                return Err(ValidationError::BadCrop);
            }
        }
        let mut ids: Vec<ObjectId> = self.objects.iter().map(|o| o.id).collect();
        ids.sort_unstable();
        for pair in ids.windows(2) {
            if let [a, b] = pair
                && a == b
            {
                return Err(ValidationError::BadId(*a));
            }
        }
        for o in &self.objects {
            if o.id.0 == 0 || o.id.0 >= self.next_id {
                return Err(ValidationError::BadId(o.id));
            }
            o.shape.validate().map_err(|why| ValidationError::BadObject(o.id, why))?;
            o.style.validate().map_err(|why| ValidationError::BadObject(o.id, why))?;
        }
        Ok(())
    }

    // ---- crate-internal mutation, used only by `Op::apply` -------------------------------

    pub(crate) const fn objects_mut(&mut self) -> &mut Vec<Object> {
        &mut self.objects
    }

    pub(crate) const fn set_crop(&mut self, crop: Option<Rect>) {
        self.crop = crop;
    }

    pub(crate) fn allocate_id(&mut self) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Ensures a re-inserted object (redo of an add) can never collide with a future id.
    pub(crate) fn reserve_id(&mut self, id: ObjectId) {
        if id.0 >= self.next_id {
            self.next_id = id.0.saturating_add(1);
        }
    }
}

/// Image dimensions are bounded by [`Limits::MAX_IMAGE_DIMENSION`] (2^20), so they are exact in f32.
#[expect(clippy::cast_precision_loss, reason = "dimensions ≤ 2^20 are exactly representable")]
fn u32_to_f32(v: u32) -> f32 {
    v as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Shape, Style};

    fn doc_with_steps() -> Document {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        for x in [10.0, 20.0, 30.0] {
            let id = doc.allocate_id();
            let shape = Shape::Step { center: Point::new(x, 10.0), radius: 5.0 };
            let style = ShapeKind::Step.default_style();
            doc.objects_mut().push(Object { id, shape, style });
        }
        doc
    }

    #[test]
    fn step_numbers_follow_z_order() {
        let mut doc = doc_with_steps();
        assert_eq!(doc.step_number(ObjectId(3)), Some(3), "third marker");
        doc.objects_mut().remove(0);
        assert_eq!(doc.step_number(ObjectId(3)), Some(2), "renumbers after deletion");
    }

    #[test]
    fn hit_test_prefers_topmost() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        for _ in 0..2 {
            let id = doc.allocate_id();
            let shape =
                Shape::Rectangle { rect: Rect::new(0.0, 0.0, 50.0, 50.0), corner_radius: 0.0 };
            doc.objects_mut().push(Object {
                id,
                shape,
                style: ShapeKind::Rectangle.default_style(),
            });
        }
        assert_eq!(doc.hit_test(Point::new(10.0, 10.0), 0.0), Some(ObjectId(2)), "top wins");
        assert_eq!(doc.hit_test(Point::new(90.0, 90.0), 0.0), None, "empty area");
    }

    #[test]
    fn validate_catches_corruption() {
        let doc = doc_with_steps();
        assert_eq!(doc.validate(), Ok(()), "fresh document is valid");

        let mut dup = doc.clone();
        if let Some(o) = dup.objects_mut().get_mut(1) {
            o.id = ObjectId(1);
        }
        assert_eq!(dup.validate(), Err(ValidationError::BadId(ObjectId(1))), "duplicate id");

        let mut crop = doc.clone();
        crop.set_crop(Some(Rect::new(50.0, 50.0, 100.0, 100.0)));
        assert_eq!(crop.validate(), Err(ValidationError::BadCrop), "crop outside image");

        let mut exhausted = doc.clone();
        exhausted.next_id = u64::MAX;
        assert!(
            matches!(exhausted.validate(), Err(ValidationError::BadId(_))),
            "next_id at the limit"
        );

        let empty = Document::new(BaseImage { width: 0, height: 10 });
        assert_eq!(empty.validate(), Err(ValidationError::ImageSize), "empty image");

        let mut bad_style = doc;
        if let Some(o) = bad_style.objects_mut().first_mut() {
            o.style = Style { stroke_width: f32::NAN, ..o.style };
        }
        assert!(matches!(bad_style.validate(), Err(ValidationError::BadObject(..))), "NaN width");
    }

    #[test]
    fn json_round_trip() {
        let doc = doc_with_steps();
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back, doc, "round trip");
    }
}
