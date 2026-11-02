//! Editor state and behaviour, independent of any UI toolkit.

use std::path::{Path, PathBuf};

use tellshot_core::{
    CoalesceKey, Color, Edit, EditError, History, ObjectId, Point, Rect, Shape, ShapeKind, Style,
    palette,
};
use tellshot_format::png_codec::{self, Effort};
use tellshot_format::{FormatError, Project, atomic_write};
use tellshot_imaging::Image;
use tellshot_render::Fonts;

/// The active tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Select and move annotations.
    Select,
    /// Draw a new annotation of this kind.
    Draw(ShapeKind),
}

/// A toolbar entry: id (used by the UI), accessible name, shortcut key and tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    /// Stable id shared with the UI.
    pub id: &'static str,
    /// Accessible name, also shown as the tooltip.
    pub label: &'static str,
    /// Single-key shortcut (lower case).
    pub key: &'static str,
    /// The tool.
    pub tool: Tool,
}

/// Every tool, in toolbar order. Shortcuts follow common editor conventions.
pub const TOOLS: [ToolSpec; 12] = [
    ToolSpec { id: "select", label: "Select", key: "v", tool: Tool::Select },
    ToolSpec { id: "arrow", label: "Arrow", key: "a", tool: Tool::Draw(ShapeKind::Arrow) },
    ToolSpec { id: "line", label: "Line", key: "l", tool: Tool::Draw(ShapeKind::Line) },
    ToolSpec {
        id: "rectangle",
        label: "Rectangle",
        key: "r",
        tool: Tool::Draw(ShapeKind::Rectangle),
    },
    ToolSpec { id: "ellipse", label: "Ellipse", key: "o", tool: Tool::Draw(ShapeKind::Ellipse) },
    ToolSpec { id: "text", label: "Text", key: "t", tool: Tool::Draw(ShapeKind::Text) },
    ToolSpec { id: "step", label: "Step number", key: "n", tool: Tool::Draw(ShapeKind::Step) },
    ToolSpec {
        id: "highlight",
        label: "Highlighter",
        key: "h",
        tool: Tool::Draw(ShapeKind::Highlight),
    },
    ToolSpec { id: "blur", label: "Blur", key: "b", tool: Tool::Draw(ShapeKind::Blur) },
    ToolSpec { id: "pixelate", label: "Pixelate", key: "p", tool: Tool::Draw(ShapeKind::Pixelate) },
    ToolSpec { id: "redact", label: "Redact", key: "x", tool: Tool::Draw(ShapeKind::Redact) },
    ToolSpec { id: "pen", label: "Pen", key: "d", tool: Tool::Draw(ShapeKind::Freehand) },
];

/// Gestures that coalesce into one undo step.
const NUDGE: CoalesceKey = CoalesceKey(u64::MAX);
const TYPING: CoalesceKey = CoalesceKey(u64::MAX - 1);
/// Hit-test tolerance in document pixels.
const HIT_TOLERANCE: f32 = 6.0;
/// A drag shorter than this is treated as a click.
const CLICK_SLOP: f32 = 3.0;

#[derive(Debug, Clone, Copy)]
enum DragMode {
    Create(ObjectId),
    Move(ObjectId),
    Idle,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    start: Point,
    last: Point,
    /// True once the pointer has left the click radius; a loop back to the start stays a drag.
    moved: bool,
    mode: DragMode,
    key: CoalesceKey,
}

/// All editor state. UI-independent.
#[derive(Debug)]
pub struct Session {
    project: Option<Project>,
    path: Option<PathBuf>,
    history: History,
    tool: Tool,
    swatch: usize,
    stroke_width: f32,
    selection: Option<ObjectId>,
    drag: Option<Drag>,
    next_gesture: u64,
    dirty: bool,
    status: String,
    fonts: Fonts,
}

impl Session {
    /// An empty session. `fonts` renders text; use [`Fonts::none`] in tests.
    #[must_use]
    pub fn new(fonts: Fonts) -> Self {
        Self {
            project: None,
            path: None,
            history: History::new(),
            tool: Tool::Draw(ShapeKind::Arrow),
            swatch: 0,
            stroke_width: 4.0,
            selection: None,
            drag: None,
            next_gesture: 1,
            dirty: false,
            status: "Ready".into(),
            fonts,
        }
    }

    // ---- Documents ---------------------------------------------------------------------

    /// Starts editing a fresh capture (unsaved).
    pub fn open_image(&mut self, image: Image, name_hint: &str) {
        self.project = Some(Project::new(image));
        self.path = None;
        self.history = History::new();
        self.selection = None;
        self.dirty = true;
        self.status = format!("Opened {name_hint}");
    }

    /// Opens a `.tellshot` project or a PNG.
    ///
    /// # Errors
    /// When the file cannot be read or is invalid; the current document is kept.
    pub fn open_path(&mut self, path: &Path) -> Result<(), FormatError> {
        let is_png = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if is_png {
            let image = png_codec::decode(&std::fs::read(path)?)?;
            self.open_image(image, &display_name(path));
            return Ok(());
        }
        self.project = Some(Project::load(path)?);
        self.path = Some(path.to_owned());
        self.history = History::new();
        self.selection = None;
        self.dirty = false;
        self.status = format!("Opened {}", display_name(path));
        Ok(())
    }

    /// Saves to `path` atomically and remembers it.
    ///
    /// # Errors
    /// When writing fails; the document stays dirty.
    pub fn save_as(&mut self, path: &Path) -> Result<(), FormatError> {
        let Some(project) = &self.project else { return Ok(()) };
        project.save(path, concat!("tellshot-editor ", env!("CARGO_PKG_VERSION")))?;
        self.path = Some(path.to_owned());
        self.dirty = false;
        self.status = format!("Saved {}", display_name(path));
        Ok(())
    }

    /// Exports a flattened PNG.
    ///
    /// # Errors
    /// When rendering or writing fails.
    pub fn export_png(&mut self, path: &Path) -> Result<(), String> {
        let image = self.render().ok_or("nothing to export")?;
        let png = png_codec::encode(&image, Effort::Small).map_err(|e| e.to_string())?;
        atomic_write(path, &png).map_err(|e| e.to_string())?;
        self.status = format!("Exported {}", display_name(path));
        Ok(())
    }

    /// Where the project was last saved.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// True when there are unsaved changes.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// True when a document is open.
    #[must_use]
    pub const fn has_document(&self) -> bool {
        self.project.is_some()
    }

    /// The flattened image with every annotation.
    #[must_use]
    pub fn render(&self) -> Option<Image> {
        let p = self.project.as_ref()?;
        tellshot_render::render(&p.document, &p.base, &self.fonts).ok()
    }

    /// The whole image with annotations, ignoring the crop, for the editor canvas: the canvas
    /// maps pointer positions 1:1 to document pixels, so it must start at (0, 0).
    #[must_use]
    pub fn render_canvas(&self) -> Option<Image> {
        let p = self.project.as_ref()?;
        let mut uncropped = p.document.clone();
        let _ignored = History::new().apply(&mut uncropped, Edit::Crop { rect: None }, None);
        tellshot_render::render(&uncropped, &p.base, &self.fonts).ok()
    }

    /// The document, if one is open.
    #[must_use]
    pub fn document(&self) -> Option<&tellshot_core::Document> {
        self.project.as_ref().map(|p| &p.document)
    }

    // ---- Tools and style ---------------------------------------------------------------

    /// The active tool.
    #[must_use]
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    /// Activates a tool by its id.
    pub fn select_tool(&mut self, id: &str) {
        if let Some(spec) = TOOLS.iter().find(|t| t.id == id) {
            self.tool = spec.tool;
            self.status = format!("{} tool", spec.label);
        }
    }

    /// Handles a single-key shortcut. Returns true when it was used.
    pub fn shortcut(&mut self, key: &str) -> bool {
        let key = key.to_lowercase();
        match TOOLS.iter().find(|t| t.key == key) {
            Some(spec) if self.has_document() => {
                self.select_tool(spec.id);
                true
            }
            _ => false,
        }
    }

    /// Index of the current colour in [`palette::SWATCHES`].
    #[must_use]
    pub const fn swatch(&self) -> usize {
        self.swatch
    }

    /// Current stroke width.
    #[must_use]
    pub const fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Picks a colour; also recolours the selection (and nothing else about it).
    pub fn choose_swatch(&mut self, index: usize) {
        let Some((_, name)) = palette::SWATCHES.get(index) else { return };
        self.swatch = index;
        self.status = format!("Colour {name}");
        let color = self.color();
        self.restyle_selection(|kind, style| recolor(kind, style, color));
    }

    /// Sets the stroke width; also applies it to the selection (and nothing else about it).
    pub fn set_stroke_width(&mut self, width: f32) {
        let width = width.clamp(1.0, 64.0);
        if (width - self.stroke_width).abs() < f32::EPSILON {
            return;
        }
        self.stroke_width = width;
        self.restyle_selection(|_, style| Style { stroke_width: width, ..style });
    }

    fn color(&self) -> Color {
        palette::SWATCHES.get(self.swatch).map_or(palette::VERMILION, |(c, _)| *c)
    }

    fn style_for(&self, kind: ShapeKind) -> Style {
        let base = kind.default_style();
        let base = match kind {
            ShapeKind::Arrow
            | ShapeKind::Line
            | ShapeKind::Rectangle
            | ShapeKind::Ellipse
            | ShapeKind::Freehand => Style { stroke_width: self.stroke_width, ..base },
            ShapeKind::Text
            | ShapeKind::Step
            | ShapeKind::Highlight
            | ShapeKind::Blur
            | ShapeKind::Pixelate
            | ShapeKind::Redact => base,
        };
        recolor(kind, base, self.color())
    }

    /// Changes the selection's style with `change`, keeping every attribute it does not touch.
    fn restyle_selection(&mut self, change: impl FnOnce(ShapeKind, Style) -> Style) {
        let Some(id) = self.selection else { return };
        let Some((kind, current)) =
            self.document().and_then(|d| d.get(id)).map(|o| (o.shape.kind(), o.style))
        else {
            return;
        };
        let style = change(kind, current);
        self.apply(Edit::SetStyle { id, style }, None);
    }

    // ---- Editing ------------------------------------------------------------------------

    fn apply(&mut self, edit: Edit, key: Option<CoalesceKey>) -> Option<ObjectId> {
        let project = self.project.as_mut()?;
        match self.history.apply(&mut project.document, edit, key) {
            Ok(Some(applied)) => {
                self.dirty = true;
                self.status.clone_from(&applied.label);
                match applied.op {
                    tellshot_core::Op::Insert { object, .. } => Some(object.id),
                    tellshot_core::Op::Remove { .. }
                    | tellshot_core::Op::Replace { .. }
                    | tellshot_core::Op::Reorder { .. }
                    | tellshot_core::Op::SetCrop { .. } => None,
                }
            }
            Ok(None) => None,
            Err(e) => {
                self.status = describe_error(&e);
                None
            }
        }
    }

    fn new_gesture(&mut self) -> CoalesceKey {
        self.next_gesture = self.next_gesture.wrapping_add(1) % (u64::MAX - 2);
        CoalesceKey(self.next_gesture)
    }

    /// Pointer pressed on the canvas at a document position.
    pub fn pointer_down(&mut self, p: Point) {
        if !self.has_document() {
            return;
        }
        let key = self.new_gesture();
        let mode = match self.tool {
            Tool::Select => {
                self.selection = self.document().and_then(|d| d.hit_test(p, HIT_TOLERANCE));
                self.selection.map_or(DragMode::Idle, DragMode::Move)
            }
            Tool::Draw(kind) => {
                let shape = shape_between(kind, p, p, self.stroke_width);
                let style = Some(self.style_for(kind));
                // Same gesture key as the reshaping that follows: drawing is one undo step.
                match self.apply(Edit::Add { shape, style }, Some(key)) {
                    Some(id) => {
                        self.selection = Some(id);
                        DragMode::Create(id)
                    }
                    None => DragMode::Idle,
                }
            }
        };
        self.drag = Some(Drag { start: p, last: p, moved: false, mode, key });
    }

    /// Pointer moved while pressed.
    pub fn pointer_move(&mut self, p: Point) {
        let Some(mut drag) = self.drag else { return };
        match drag.mode {
            DragMode::Move(id) => {
                let (dx, dy) = (p.x - drag.last.x, p.y - drag.last.y);
                self.apply(Edit::Translate { id, dx, dy }, Some(drag.key));
            }
            DragMode::Create(id) => {
                let current = self.document().and_then(|d| d.get(id)).map(|o| o.shape.clone());
                if let Some(current) = current {
                    let shape = match current {
                        Shape::Freehand { mut points } => {
                            points.push(p);
                            Shape::Freehand { points }
                        }
                        other @ (Shape::Arrow { .. }
                        | Shape::Line { .. }
                        | Shape::Rectangle { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Text { .. }
                        | Shape::Step { .. }
                        | Shape::Highlight { .. }
                        | Shape::Blur { .. }
                        | Shape::Pixelate { .. }
                        | Shape::Redact { .. }) => {
                            shape_between(other.kind(), drag.start, p, self.stroke_width)
                        }
                    };
                    self.apply(Edit::SetShape { id, shape }, Some(drag.key));
                }
            }
            DragMode::Idle => {}
        }
        drag.last = p;
        drag.moved |= drag.start.distance(p) >= CLICK_SLOP;
        self.drag = Some(drag);
    }

    /// Pointer released. A click without a drag creates a default-sized shape at the point.
    pub fn pointer_up(&mut self, p: Point) {
        self.pointer_move(p);
        let Some(drag) = self.drag.take() else { return };
        if let DragMode::Create(id) = drag.mode
            && !drag.moved
            && let Some(kind) = self.document().and_then(|d| d.get(id)).map(|o| o.shape.kind())
        {
            let shape = default_shape(kind, drag.start, self.stroke_width);
            self.apply(Edit::SetShape { id, shape }, Some(drag.key));
        }
        self.history.seal();
    }

    /// Inserts the current tool's shape at the centre of the view (keyboard drawing).
    pub fn insert_at_center(&mut self) {
        let Tool::Draw(kind) = self.tool else { return };
        let Some(center) = self.document().map(|d| d.visible_rect().center()) else { return };
        let shape = default_shape(kind, center, self.stroke_width);
        let style = Some(self.style_for(kind));
        if let Some(id) = self.apply(Edit::Add { shape, style }, None) {
            self.selection = Some(id);
            self.history.seal();
        }
    }

    /// Moves the selection by an offset; consecutive nudges are one undo step.
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        if let Some(id) = self.selection {
            self.apply(Edit::Translate { id, dx, dy }, Some(NUDGE));
        }
    }

    /// Deletes the selection.
    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selection.take() {
            self.apply(Edit::Delete { id }, None);
        }
    }

    /// Clears the selection.
    pub fn deselect(&mut self) {
        self.selection = None;
        self.history.seal();
    }

    /// Replaces the text of the selected text annotation; typing coalesces into one step.
    pub fn set_selected_text(&mut self, text: &str) {
        let Some(id) = self.selection else { return };
        let Some(Shape::Text { origin, font_size, .. }) =
            self.document().and_then(|d| d.get(id)).map(|o| o.shape.clone())
        else {
            return;
        };
        let shape = Shape::Text { origin, text: text.to_owned(), font_size };
        self.apply(Edit::SetShape { id, shape }, Some(TYPING));
    }

    /// Undoes the last step.
    pub fn undo(&mut self) {
        let Some(project) = self.project.as_mut() else { return };
        match self.history.undo(&mut project.document) {
            Ok(Some(label)) => {
                self.dirty = true;
                self.status = format!("Undid {label}");
                self.drop_stale_selection();
            }
            Ok(None) => self.status = "Nothing to undo".into(),
            Err(e) => self.status = format!("Undo failed: {e}"),
        }
    }

    /// Redoes the last undone step.
    pub fn redo(&mut self) {
        let Some(project) = self.project.as_mut() else { return };
        match self.history.redo(&mut project.document) {
            Ok(Some(label)) => {
                self.dirty = true;
                self.status = format!("Redid {label}");
                self.drop_stale_selection();
            }
            Ok(None) => self.status = "Nothing to redo".into(),
            Err(e) => self.status = format!("Redo failed: {e}"),
        }
    }

    fn drop_stale_selection(&mut self) {
        if let Some(id) = self.selection
            && self.document().and_then(|d| d.get(id)).is_none()
        {
            self.selection = None;
        }
    }

    // ---- Queries for the UI ---------------------------------------------------------------

    /// The selected object.
    #[must_use]
    pub const fn selection(&self) -> Option<ObjectId> {
        self.selection
    }

    /// Selects by position in [`Session::annotation_rows`] (top-most first).
    pub fn select_row(&mut self, row: usize) {
        let id = self.document().and_then(|d| d.objects().iter().rev().nth(row)).map(|o| o.id);
        if id.is_some() && id != self.selection {
            self.selection = id;
            self.history.seal();
        }
    }

    /// Row of the selection in [`Session::annotation_rows`].
    #[must_use]
    pub fn selected_row(&self) -> Option<usize> {
        let id = self.selection?;
        self.document()?.objects().iter().rev().position(|o| o.id == id)
    }

    /// One readable line per annotation, top-most first.
    #[must_use]
    pub fn annotation_rows(&self) -> Vec<String> {
        let Some(doc) = self.document() else { return Vec::new() };
        doc.objects().iter().rev().map(|o| o.describe(doc.step_number(o.id))).collect()
    }

    /// Bounds of the selection, for the selection outline.
    #[must_use]
    pub fn selection_bounds(&self) -> Option<Rect> {
        let id = self.selection?;
        Some(self.document()?.get(id)?.shape.bounds())
    }

    /// Text of the selection when it is a text annotation.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let id = self.selection?;
        if let Shape::Text { text, .. } = &self.document()?.get(id)?.shape {
            Some(text.clone())
        } else {
            None
        }
    }

    /// Label of the step undo would revert.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    /// Label of the step redo would re-apply.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// The latest status message.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Shows a message in the status bar.
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    /// Window title: file name, unsaved marker, app name.
    #[must_use]
    pub fn title(&self) -> String {
        let name = self.path.as_deref().map_or_else(|| "Untitled".into(), display_name);
        let mark = if self.dirty { " •" } else { "" };
        if self.has_document() { format!("{name}{mark} — Tellshot") } else { "Tellshot".into() }
    }
}

/// Applies a colour the way each kind uses it, leaving the rest of the style alone.
fn recolor(kind: ShapeKind, style: Style, color: Color) -> Style {
    match kind {
        ShapeKind::Arrow
        | ShapeKind::Line
        | ShapeKind::Rectangle
        | ShapeKind::Ellipse
        | ShapeKind::Freehand
        | ShapeKind::Text => Style { stroke: color, ..style },
        ShapeKind::Step => Style { fill: Some(color), ..style },
        ShapeKind::Highlight => {
            Style { stroke: color, fill: Some(color.with_alpha(0x66)), ..style }
        }
        // Redaction is always opaque black; blur and pixelate have no colour.
        ShapeKind::Blur | ShapeKind::Pixelate | ShapeKind::Redact => style,
    }
}

fn describe_error(e: &EditError) -> String {
    format!("Could not apply: {e}")
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The shape a drag from `a` to `b` describes.
fn shape_between(kind: ShapeKind, a: Point, b: Point, width: f32) -> Shape {
    let rect = Rect::from_points(a, b);
    match kind {
        ShapeKind::Arrow => Shape::Arrow { from: a, to: b },
        ShapeKind::Line => Shape::Line { from: a, to: b },
        ShapeKind::Rectangle => Shape::Rectangle { rect, corner_radius: 4.0 },
        ShapeKind::Ellipse => Shape::Ellipse { rect },
        ShapeKind::Highlight => Shape::Highlight { rect },
        ShapeKind::Blur => Shape::Blur { rect, radius: 12.0 },
        ShapeKind::Pixelate => Shape::Pixelate { rect, block_size: 12.0 },
        ShapeKind::Redact => Shape::Redact { rect },
        ShapeKind::Step => Shape::Step { center: b, radius: width.mul_add(2.0, 10.0) },
        ShapeKind::Text => {
            Shape::Text { origin: a, text: "Text".into(), font_size: width.mul_add(3.0, 12.0) }
        }
        ShapeKind::Freehand => Shape::Freehand { points: vec![a] },
    }
}

/// A sensible default-sized shape centred on `at`, for clicks and keyboard insertion.
fn default_shape(kind: ShapeKind, at: Point, stroke: f32) -> Shape {
    let (width, height) = match kind {
        ShapeKind::Highlight => (160.0, 24.0),
        ShapeKind::Redact => (160.0, 20.0),
        ShapeKind::Arrow
        | ShapeKind::Line
        | ShapeKind::Rectangle
        | ShapeKind::Ellipse
        | ShapeKind::Text
        | ShapeKind::Step
        | ShapeKind::Blur
        | ShapeKind::Pixelate
        | ShapeKind::Freehand => (120.0, 80.0),
    };
    let top_left = at.offset(-width / 2.0, -height / 2.0);
    let bottom_right = at.offset(width / 2.0, height / 2.0);
    match kind {
        ShapeKind::Arrow | ShapeKind::Line => {
            shape_between(kind, at.offset(-width / 2.0, height / 2.0), at, stroke)
        }
        ShapeKind::Step | ShapeKind::Text => shape_between(kind, at, at, stroke),
        ShapeKind::Freehand => Shape::Freehand {
            points: vec![top_left, at.offset(-width / 4.0, height / 4.0), at, bottom_right],
        },
        ShapeKind::Rectangle
        | ShapeKind::Ellipse
        | ShapeKind::Highlight
        | ShapeKind::Blur
        | ShapeKind::Pixelate
        | ShapeKind::Redact => shape_between(kind, top_left, bottom_right, stroke),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new(Fonts::none());
        s.open_image(Image::new(400, 300, [240, 240, 240, 255]).unwrap(), "test");
        s
    }

    #[test]
    fn drawing_a_rectangle_is_one_undo_step() {
        let mut s = session();
        s.select_tool("rectangle");
        s.pointer_down(Point::new(10.0, 10.0));
        for i in 1..20_u8 {
            s.pointer_move(Point::new(
                f32::from(i).mul_add(5.0, 10.0),
                f32::from(i).mul_add(3.0, 10.0),
            ));
        }
        s.pointer_up(Point::new(120.0, 80.0));
        assert_eq!(s.annotation_rows(), ["Rectangle #1 at (10, 10), 110 by 70"], "drawn");
        s.undo();
        assert!(s.annotation_rows().is_empty(), "a whole drag undoes in one step");
        s.redo();
        assert_eq!(s.annotation_rows().len(), 1, "and redoes in one step");
    }

    #[test]
    fn click_without_drag_makes_a_default_shape() {
        let mut s = session();
        s.select_tool("ellipse");
        s.pointer_down(Point::new(200.0, 150.0));
        s.pointer_up(Point::new(201.0, 150.0));
        let b = s.selection_bounds().unwrap();
        assert!((b.width - 120.0).abs() < 0.01 && (b.height - 80.0).abs() < 0.01, "{b:?}");
    }

    #[test]
    fn everything_works_from_the_keyboard() {
        let mut s = session();
        assert!(s.shortcut("x"), "x selects redact");
        s.insert_at_center();
        s.nudge(1.0, 0.0);
        s.nudge(1.0, 0.0);
        s.nudge(0.0, 10.0);
        let b = s.selection_bounds().unwrap();
        assert_eq!((b.x, b.y), (122.0, 150.0), "inserted at centre, nudged");
        s.undo();
        assert_eq!(
            s.selection_bounds().map(|r| (r.x, r.y)),
            Some((120.0, 140.0)),
            "nudges undo together"
        );
        s.delete_selected();
        assert!(s.annotation_rows().is_empty(), "deleted");
        assert!(!s.shortcut("q"), "unknown keys are not swallowed");
    }

    #[test]
    fn selecting_moving_and_restyling() {
        let mut s = session();
        s.select_tool("arrow");
        s.pointer_down(Point::new(10.0, 10.0));
        s.pointer_up(Point::new(100.0, 100.0));
        s.select_tool("select");
        s.deselect();
        s.pointer_down(Point::new(55.0, 55.0));
        assert!(s.selection().is_some(), "hit the arrow shaft");
        s.pointer_move(Point::new(65.0, 55.0));
        s.pointer_up(Point::new(75.0, 55.0));
        assert_eq!(s.annotation_rows(), ["Arrow #1 from (30, 10) to (120, 100)"], "moved by 20");
        s.choose_swatch(1);
        let style = s.document().unwrap().objects()[0].style;
        assert_eq!(style.stroke, palette::BLUE, "recoloured");
        assert_eq!(s.undo_label(), Some("Restyle arrow"), "undo label");
    }

    #[test]
    fn restyling_changes_only_the_touched_attribute() {
        let mut s = session();
        s.select_tool("rectangle");
        s.set_stroke_width(10.0);
        s.insert_at_center();
        s.set_stroke_width(10.0);
        s.choose_swatch(2);
        let style = s.document().unwrap().objects()[0].style;
        assert_eq!((style.stroke, style.stroke_width), (palette::SKY_BLUE, 10.0), "colour only");
        s.set_stroke_width(3.0);
        let style = s.document().unwrap().objects()[0].style;
        assert_eq!((style.stroke, style.stroke_width), (palette::SKY_BLUE, 3.0), "width only");
    }

    #[test]
    fn a_pen_loop_that_ends_at_its_start_stays_a_loop() {
        let mut s = session();
        s.select_tool("pen");
        s.pointer_down(Point::new(100.0, 100.0));
        for p in [(150.0, 100.0), (150.0, 150.0), (100.0, 150.0), (101.0, 101.0)] {
            s.pointer_move(Point::new(p.0, p.1));
        }
        s.pointer_up(Point::new(100.5, 100.0));
        assert_eq!(s.annotation_rows(), ["Pen stroke #1 with 6 points"], "kept the drawn loop");
    }

    #[test]
    fn text_edits_coalesce_and_title_tracks_changes() {
        let mut s = session();
        s.select_tool("text");
        s.insert_at_center();
        for t in ["H", "He", "Hel", "Hello"] {
            s.set_selected_text(t);
        }
        assert_eq!(s.selected_text().as_deref(), Some("Hello"), "text");
        s.undo();
        assert_eq!(s.selected_text().as_deref(), Some("Text"), "typing is one undo step");
        assert!(s.title().starts_with("Untitled •"), "unsaved marker: {}", s.title());

        let dir = std::env::temp_dir().join(format!("tellshot-editor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.tellshot");
        s.save_as(&path).unwrap();
        assert_eq!(s.title(), "t.tellshot — Tellshot", "saved");
        let mut reopened = Session::new(Fonts::none());
        reopened.open_path(&path).unwrap();
        assert_eq!(reopened.annotation_rows(), s.annotation_rows(), "round trip");
        let _cleanup = std::fs::remove_dir_all(&dir);
    }
}
