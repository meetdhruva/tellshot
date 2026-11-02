//! Undo/redo with gesture coalescing.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{Document, Edit, EditError, Limits, Op, OpError};

/// Identifies one continuous user gesture (for example a drag). Consecutive edits with the same
/// key on the same object merge into one undo step, so undo reverts the whole drag at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoalesceKey(pub u64);

/// The result of a successful [`History::apply`].
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The op that was applied.
    pub op: Op,
    /// Human-readable label, for example "Add arrow".
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    op: Op,
    label: String,
    key: Option<CoalesceKey>,
}

/// The undo and redo stacks of one document.
#[derive(Debug, Clone, Default)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
}

impl History {
    /// An empty history.
    #[must_use]
    pub const fn new() -> Self {
        Self { undo: VecDeque::new(), redo: Vec::new() }
    }

    /// Validates `edit`, applies it to `doc` and records it. Clears the redo stack.
    ///
    /// Returns `Ok(None)` when the edit is valid but changes nothing (nothing is recorded).
    ///
    /// # Errors
    /// [`EditError`] when the edit is invalid; the document is then unchanged.
    pub fn apply(
        &mut self,
        doc: &mut Document,
        edit: Edit,
        key: Option<CoalesceKey>,
    ) -> Result<Option<Applied>, EditError> {
        let label = edit.label(doc);
        let op = edit.resolve(doc)?;
        if op.is_noop() {
            return Ok(None);
        }
        op.apply(doc).map_err(|OpError::Mismatch| EditError::Invalid("internal state mismatch"))?;
        self.redo.clear();
        if !self.try_coalesce(&op, key) {
            if self.undo.len() >= Limits::MAX_UNDO_DEPTH {
                self.undo.pop_front();
            }
            self.undo.push_back(Entry { op: op.clone(), label: label.clone(), key });
        }
        Ok(Some(Applied { op, label }))
    }

    /// Merges a `Replace` into the previous entry when both belong to the same gesture.
    fn try_coalesce(&mut self, op: &Op, key: Option<CoalesceKey>) -> bool {
        let (Some(key), Some(last)) = (key, self.undo.back_mut()) else {
            return false;
        };
        if last.key != Some(key) {
            return false;
        }
        let merged = match (&mut last.op, op) {
            // Dragging an existing object: keep the first `before`, take the latest `after`.
            (
                Op::Replace { index: i, after: prev_after, .. },
                Op::Replace { index: j, after, .. },
            ) if i == j && prev_after.id == after.id => {
                prev_after.clone_from(after);
                true
            }
            // Drawing a new object: the insert absorbs the reshaping that follows it.
            (Op::Insert { index: i, object }, Op::Replace { index: j, after, .. })
                if i == j && object.id == after.id =>
            {
                object.clone_from(after);
                true
            }
            _ => false,
        };
        // A drag that ends where it started leaves nothing to undo.
        if merged && last.op.is_noop() {
            self.undo.pop_back();
        }
        merged
    }

    /// Undoes the most recent entry. Returns its label, or `None` when there is nothing to undo.
    ///
    /// # Errors
    /// [`OpError`] if the document was changed behind the history's back (a bug).
    pub fn undo(&mut self, doc: &mut Document) -> Result<Option<String>, OpError> {
        let Some(entry) = self.undo.pop_back() else {
            return Ok(None);
        };
        if let Err(e) = entry.op.inverse().apply(doc) {
            self.undo.push_back(entry);
            return Err(e);
        }
        let label = entry.label.clone();
        self.redo.push(Entry { key: None, ..entry });
        Ok(Some(label))
    }

    /// Redoes the most recently undone entry. Returns its label, or `None` when there is none.
    ///
    /// # Errors
    /// [`OpError`] if the document was changed behind the history's back (a bug).
    pub fn redo(&mut self, doc: &mut Document) -> Result<Option<String>, OpError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        if let Err(e) = entry.op.apply(doc) {
            self.redo.push(entry);
            return Err(e);
        }
        let label = entry.label.clone();
        self.undo.push_back(entry);
        Ok(Some(label))
    }

    /// Label of the entry [`History::undo`] would revert.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|e| e.label.as_str())
    }

    /// Label of the entry [`History::redo`] would re-apply.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    /// Ends the current gesture so the next edit starts a new undo step.
    pub fn seal(&mut self) {
        if let Some(last) = self.undo.back_mut() {
            last.key = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BaseImage, ObjectId, Point, Rect, Shape};

    fn arrow() -> Edit {
        Edit::Add {
            shape: Shape::Arrow { from: Point::new(0.0, 0.0), to: Point::new(10.0, 10.0) },
            style: None,
        }
    }

    #[test]
    fn a_drag_is_one_undo_step() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        let mut history = History::new();
        history.apply(&mut doc, arrow(), None).unwrap();
        let before = doc.clone();
        let drag = Some(CoalesceKey(1));
        for _ in 0..10 {
            history
                .apply(&mut doc, Edit::Translate { id: ObjectId(1), dx: 1.0, dy: 2.0 }, drag)
                .unwrap();
        }
        assert_eq!(history.undo_label(), Some("Move arrow"), "label");
        history.undo(&mut doc).unwrap();
        assert_eq!(doc, before, "one undo reverts the whole drag");
    }

    #[test]
    fn drawing_a_shape_is_one_undo_step() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        let mut history = History::new();
        let gesture = Some(CoalesceKey(7));
        let rect =
            |w: f32| Shape::Rectangle { rect: Rect::new(10.0, 10.0, w, w), corner_radius: 0.0 };
        history.apply(&mut doc, Edit::Add { shape: rect(0.0), style: None }, gesture).unwrap();
        for w in [5.0, 10.0, 20.0] {
            history
                .apply(&mut doc, Edit::SetShape { id: ObjectId(1), shape: rect(w) }, gesture)
                .unwrap();
        }
        assert_eq!(history.undo_label(), Some("Add rectangle"), "one entry, labelled as the add");
        history.undo(&mut doc).unwrap();
        assert!(doc.objects().is_empty(), "one undo removes the drawn shape");
        history.redo(&mut doc).unwrap();
        assert_eq!(
            doc.objects().first().map(|o| o.shape.bounds().width),
            Some(20.0),
            "redo restores the final size"
        );
    }

    #[test]
    fn new_edits_clear_redo_and_noops_are_ignored() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        let mut history = History::new();
        history.apply(&mut doc, arrow(), None).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(history.redo_label(), Some("Add arrow"), "redo available");
        history.apply(&mut doc, arrow(), None).unwrap();
        assert_eq!(history.redo_label(), None, "redo cleared by a new edit");
        let noop = history
            .apply(&mut doc, Edit::Translate { id: ObjectId(2), dx: 0.0, dy: 0.0 }, None)
            .unwrap();
        assert_eq!(noop, None, "no-op is not recorded");
    }

    #[test]
    fn rejected_edits_leave_the_document_untouched() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        let mut history = History::new();
        let before = doc.clone();
        let bad = Edit::Add {
            shape: Shape::Redact { rect: Rect::new(f32::NAN, 0.0, 1.0, 1.0) },
            style: None,
        };
        assert!(history.apply(&mut doc, bad, None).is_err(), "NaN rejected");
        assert!(
            history.apply(&mut doc, Edit::Delete { id: ObjectId(9) }, None).is_err(),
            "missing id"
        );
        assert_eq!(doc, before, "unchanged");
        assert_eq!(history.undo_label(), None, "nothing recorded");
    }

    #[test]
    fn redo_does_not_reuse_ids() {
        let mut doc = Document::new(BaseImage { width: 100, height: 100 });
        let mut history = History::new();
        history.apply(&mut doc, arrow(), None).unwrap();
        history.undo(&mut doc).unwrap();
        history.redo(&mut doc).unwrap();
        let added = history.apply(&mut doc, arrow(), None).unwrap().unwrap();
        assert!(
            matches!(added.op, Op::Insert { ref object, .. } if object.id == ObjectId(2)),
            "fresh id"
        );
        assert_eq!(doc.validate(), Ok(()), "valid");
    }
}
