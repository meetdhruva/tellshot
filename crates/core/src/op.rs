//! Resolved, invertible operations. Undo is "apply the inverse".

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{Document, Object, Rect};

/// A fully resolved change to a [`Document`]. Every op carries enough data to be inverted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Insert `object` at z-index `index`.
    Insert {
        /// Z-index to insert at.
        index: usize,
        /// The object.
        object: Object,
    },
    /// Remove `object`, which must be at z-index `index`.
    Remove {
        /// Z-index of the object.
        index: usize,
        /// The object being removed (kept so the op can be inverted).
        object: Object,
    },
    /// Replace the object at `index`; `before` must match what is there.
    Replace {
        /// Z-index of the object.
        index: usize,
        /// The object as it is now.
        before: Object,
        /// The object as it will be.
        after: Object,
    },
    /// Move the object at z-index `from` to z-index `to`.
    Reorder {
        /// Current z-index.
        from: usize,
        /// New z-index.
        to: usize,
    },
    /// Change the crop.
    SetCrop {
        /// Current crop.
        before: Option<Rect>,
        /// New crop.
        after: Option<Rect>,
    },
}

/// An op did not match the document it was applied to. This means a bug, never user error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpError {
    /// The z-index is out of range or holds a different object than expected.
    Mismatch,
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation does not match the document state")
    }
}

impl core::error::Error for OpError {}

impl Op {
    /// The op that undoes this one.
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self.clone() {
            Self::Insert { index, object } => Self::Remove { index, object },
            Self::Remove { index, object } => Self::Insert { index, object },
            Self::Replace { index, before, after } => {
                Self::Replace { index, before: after, after: before }
            }
            Self::Reorder { from, to } => Self::Reorder { from: to, to: from },
            Self::SetCrop { before, after } => Self::SetCrop { before: after, after: before },
        }
    }

    /// Applies the op after checking its preconditions. On error the document is unchanged.
    ///
    /// # Errors
    /// [`OpError::Mismatch`] when the document is not in the state the op expects.
    pub fn apply(&self, doc: &mut Document) -> Result<(), OpError> {
        match self {
            Self::Insert { index, object } => {
                if *index > doc.objects().len() || doc.get(object.id).is_some() {
                    return Err(OpError::Mismatch);
                }
                doc.reserve_id(object.id);
                doc.objects_mut().insert(*index, object.clone());
            }
            Self::Remove { index, object } => {
                if doc.objects().get(*index) != Some(object) {
                    return Err(OpError::Mismatch);
                }
                doc.objects_mut().remove(*index);
            }
            Self::Replace { index, before, after } => {
                let slot = doc.objects_mut().get_mut(*index).ok_or(OpError::Mismatch)?;
                if slot != before || before.id != after.id {
                    return Err(OpError::Mismatch);
                }
                *slot = after.clone();
            }
            Self::Reorder { from, to } => {
                let len = doc.objects().len();
                if *from >= len || *to >= len {
                    return Err(OpError::Mismatch);
                }
                let object = doc.objects_mut().remove(*from);
                doc.objects_mut().insert(*to, object);
            }
            Self::SetCrop { before, after } => {
                if doc.crop() != *before {
                    return Err(OpError::Mismatch);
                }
                doc.set_crop(*after);
            }
        }
        Ok(())
    }

    /// True when applying the op would leave the document unchanged.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        match self {
            Self::Insert { .. } | Self::Remove { .. } => false,
            Self::Replace { before, after, .. } => before == after,
            Self::Reorder { from, to } => from == to,
            Self::SetCrop { before, after } => before == after,
        }
    }
}
