//! Edits: what a user, script or AI agent asks for. Resolved into invertible [`Op`]s.

use alloc::format;
use alloc::string::String;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::geometry::valid_coordinate;
use crate::{Document, Limits, Object, ObjectId, Op, Rect, Shape, Style};

/// Where to move an object in the z-order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Arrange {
    /// On top of everything.
    Front,
    /// Below everything.
    Back,
    /// One step up.
    Forward,
    /// One step down.
    Backward,
}

/// A request to change a document. This is the public editing API for the editor UI, the
/// CLI (`tellshot edit`) and AI agents (the MCP `apply_edits` tool) alike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Add an object on top. Omit `style` to use the default for its kind.
    Add {
        /// Geometry of the new object.
        shape: Shape,
        /// Paint; defaults to the kind's default style.
        #[serde(default)]
        style: Option<Style>,
    },
    /// Delete an object.
    Delete {
        /// Object to delete.
        id: ObjectId,
    },
    /// Move an object by an offset.
    Translate {
        /// Object to move.
        id: ObjectId,
        /// Horizontal offset in document pixels.
        dx: f32,
        /// Vertical offset in document pixels.
        dy: f32,
    },
    /// Replace an object's geometry (resize, edit text). The kind must stay the same.
    SetShape {
        /// Object to change.
        id: ObjectId,
        /// New geometry.
        shape: Shape,
    },
    /// Replace an object's paint.
    SetStyle {
        /// Object to change.
        id: ObjectId,
        /// New paint.
        style: Style,
    },
    /// Change an object's position in the z-order.
    Arrange {
        /// Object to move.
        id: ObjectId,
        /// Where to move it.
        to: Arrange,
    },
    /// Set or clear the crop.
    Crop {
        /// New crop, or `null` to show the whole image.
        rect: Option<Rect>,
    },
}

/// Why an edit was rejected. The document is unchanged when this is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// No object has this id.
    NotFound(ObjectId),
    /// Values are out of range (NaN, infinite, negative sizes, too long, ...).
    Invalid(&'static str),
    /// `SetShape` tried to change an object's kind.
    KindMismatch(ObjectId),
    /// The document already holds [`Limits::MAX_OBJECTS`] objects.
    TooManyObjects,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "no object with id {id}"),
            Self::Invalid(why) => write!(f, "invalid edit: {why}"),
            Self::KindMismatch(id) => {
                write!(f, "object {id} cannot change kind; delete and add instead")
            }
            Self::TooManyObjects => {
                write!(f, "documents are limited to {} objects", Limits::MAX_OBJECTS)
            }
        }
    }
}

impl core::error::Error for EditError {}

impl Edit {
    /// A short label for the undo menu, for example "Add arrow" or "Delete".
    #[must_use]
    pub fn label(&self, doc: &Document) -> String {
        let kind = |id: &ObjectId| doc.get(*id).map_or("object", |o| o.shape.kind().label());
        match self {
            Self::Add { shape, .. } => format!("Add {}", shape.kind().label()),
            Self::Delete { id } => format!("Delete {}", kind(id)),
            Self::Translate { id, .. } => format!("Move {}", kind(id)),
            Self::SetShape { id, .. } => format!("Edit {}", kind(id)),
            Self::SetStyle { id, .. } => format!("Restyle {}", kind(id)),
            Self::Arrange { id, .. } => format!("Arrange {}", kind(id)),
            Self::Crop { .. } => String::from("Crop"),
        }
    }

    /// Validates the edit against `doc` and produces the op that performs it.
    /// Allocates an id for `Add`, so call it only when the op will be applied.
    pub(crate) fn resolve(self, doc: &mut Document) -> Result<Op, EditError> {
        let find = |doc: &Document, id: ObjectId| {
            let index = doc.index_of(id).ok_or(EditError::NotFound(id))?;
            let object = doc.objects().get(index).cloned().ok_or(EditError::NotFound(id))?;
            Ok::<_, EditError>((index, object))
        };
        match self {
            Self::Add { shape, style } => {
                if doc.objects().len() >= Limits::MAX_OBJECTS {
                    return Err(EditError::TooManyObjects);
                }
                shape.validate().map_err(EditError::Invalid)?;
                let style = style.unwrap_or_else(|| shape.kind().default_style());
                style.validate().map_err(EditError::Invalid)?;
                let object = Object { id: doc.allocate_id(), shape, style };
                Ok(Op::Insert { index: doc.objects().len(), object })
            }
            Self::Delete { id } => {
                let (index, object) = find(doc, id)?;
                Ok(Op::Remove { index, object })
            }
            Self::Translate { id, dx, dy } => {
                if !valid_coordinate(dx) || !valid_coordinate(dy) {
                    return Err(EditError::Invalid("offset is out of range"));
                }
                let (index, before) = find(doc, id)?;
                let after = Object { shape: before.shape.translated(dx, dy), ..before };
                after.shape.validate().map_err(EditError::Invalid)?;
                Ok(Op::Replace { index, before, after })
            }
            Self::SetShape { id, shape } => {
                let (index, before) = find(doc, id)?;
                if before.shape.kind() != shape.kind() {
                    return Err(EditError::KindMismatch(id));
                }
                shape.validate().map_err(EditError::Invalid)?;
                let after = Object { shape, ..before };
                Ok(Op::Replace { index, before, after })
            }
            Self::SetStyle { id, style } => {
                style.validate().map_err(EditError::Invalid)?;
                let (index, before) = find(doc, id)?;
                let after = Object { style, ..before.clone() };
                Ok(Op::Replace { index, before, after })
            }
            Self::Arrange { id, to } => {
                let (from, _) = find(doc, id)?;
                let last = doc.objects().len().saturating_sub(1);
                let to = match to {
                    Arrange::Front => last,
                    Arrange::Back => 0,
                    Arrange::Forward => from.saturating_add(1).min(last),
                    Arrange::Backward => from.saturating_sub(1),
                };
                Ok(Op::Reorder { from, to })
            }
            Self::Crop { rect } => {
                if let Some(r) = rect {
                    let image = doc.base().rect();
                    let clipped = r
                        .intersection(image)
                        .ok_or(EditError::Invalid("crop is outside the image"))?;
                    if !r.is_valid() || clipped.width < 1.0 || clipped.height < 1.0 {
                        return Err(EditError::Invalid("crop is smaller than one pixel"));
                    }
                    return Ok(Op::SetCrop { before: doc.crop(), after: Some(clipped) });
                }
                Ok(Op::SetCrop { before: doc.crop(), after: None })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_parse_from_agent_json() {
        let json = r##"[
            {"op":"add","shape":{"kind":"arrow","from":{"x":10,"y":10},"to":{"x":200,"y":120}}},
            {"op":"add","shape":{"kind":"redact","rect":{"x":0,"y":0,"width":50,"height":20}}},
            {"op":"set_style","id":1,"style":{"stroke":"#0072b2","stroke_width":6}},
            {"op":"arrange","id":1,"to":"front"},
            {"op":"crop","rect":null}
        ]"##;
        let edits: Vec<Edit> = serde_json::from_str(json).unwrap();
        assert_eq!(edits.len(), 5, "all edits parsed");
        assert!(
            serde_json::from_str::<Edit>(r#"{"op":"delete","id":1,"force":true}"#).is_err(),
            "unknown field rejected"
        );
    }
}
