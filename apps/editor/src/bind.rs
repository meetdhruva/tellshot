//! Wires the Slint window to the [`Session`]: callbacks in, property snapshots out.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{
    ComponentHandle as _, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};
use tellshot_core::{Point, palette};
use tellshot_render::Fonts;

use crate::session::{Session, TOOLS, Tool};
use crate::ui::{AppWindow, Icons, SwatchInfo, ToolInfo};

type Shared = Rc<RefCell<Session>>;

/// Opens the editor window, optionally with a file, and runs the event loop.
///
/// # Errors
/// When the window cannot be created (no display, no usable backend).
pub fn launch(path: Option<PathBuf>) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let session = attach(&window, Fonts::system());
    if let Some(path) = path {
        let result = session.borrow_mut().open_path(&path);
        if let Err(e) = result {
            session.borrow_mut().set_status(format!("Could not open {}: {e}", path.display()));
        }
        refresh(&window, &session);
    }
    window.run()
}

/// Connects a window to a new session and returns the session. Used by `launch` and UI tests.
#[must_use]
pub fn attach(window: &AppWindow, fonts: Fonts) -> Rc<RefCell<Session>> {
    let session: Shared = Rc::new(RefCell::new(Session::new(fonts)));
    install_models(window);
    wire_callbacks(window, &session);
    wire_close(window, &session);
    refresh(window, &session);
    session
}

/// Static models: tools, swatches, version.
fn install_models(window: &AppWindow) {
    window.set_version(env!("CARGO_PKG_VERSION").into());
    let icons = window.global::<Icons<'_>>();
    window.set_tools(ModelRc::new(VecModel::from(
        TOOLS
            .iter()
            .map(|t| ToolInfo {
                id: t.id.into(),
                label: t.label.into(),
                shortcut: t.key.to_uppercase().into(),
                icon: icon_for(&icons, t.id),
            })
            .collect::<Vec<_>>(),
    )));
    window.set_swatches(ModelRc::new(VecModel::from(
        palette::SWATCHES
            .iter()
            .map(|(c, name)| SwatchInfo {
                label: (*name).into(),
                swatch: slint::Color::from_argb_u8(c.a, c.r, c.g, c.b),
            })
            .collect::<Vec<_>>(),
    )));
}

fn wire_callbacks(window: &AppWindow, session: &Shared) {
    // Every handler mutates the session, releases the borrow, then refreshes the UI.
    macro_rules! wire {
        ($on:ident, [$($arg:ident),*], $body:expr) => {{
            let s = Rc::clone(session);
            let w = window.as_weak();
            window.$on(move |$($arg),*| {
                {
                    let mut session = s.borrow_mut();
                    let f = $body;
                    f(&mut *session);
                }
                if let Some(w) = w.upgrade() {
                    refresh(&w, &s);
                }
            });
        }};
    }

    wire!(on_select_tool, [id], |s: &mut Session| s.select_tool(&id));
    wire!(on_pointer_down, [x, y], |s: &mut Session| s.pointer_down(Point::new(x, y)));
    wire!(on_pointer_move, [x, y], |s: &mut Session| s.pointer_move(Point::new(x, y)));
    wire!(on_pointer_up, [x, y], |s: &mut Session| s.pointer_up(Point::new(x, y)));
    wire!(on_insert_shape, [], |s: &mut Session| s.insert_at_center());
    wire!(on_nudge, [dx, dy], |s: &mut Session| s.nudge(dx, dy));
    wire!(on_undo, [], |s: &mut Session| s.undo());
    wire!(on_redo, [], |s: &mut Session| s.redo());
    wire!(on_delete_selected, [], |s: &mut Session| s.delete_selected());
    wire!(on_deselect, [], |s: &mut Session| s.deselect());
    wire!(on_width_changed, [v], |s: &mut Session| s.set_stroke_width(v));
    wire!(on_text_edited, [t], |s: &mut Session| s.set_selected_text(&t));
    wire!(on_swatch_chosen, [i], |s: &mut Session| {
        if let Ok(i) = usize::try_from(i) {
            s.choose_swatch(i);
        }
    });
    wire!(on_annotation_chosen, [row], |s: &mut Session| {
        if let Ok(row) = usize::try_from(row) {
            s.select_row(row);
        }
    });
    // File and capture actions show dialogs, so they borrow the session only briefly and never
    // across a modal dialog (a nested event loop could otherwise re-enter and panic).
    macro_rules! action {
        ($on:ident, $action:expr) => {{
            let s = Rc::clone(session);
            let w = window.as_weak();
            window.$on(move || {
                $action(&s);
                if let Some(w) = w.upgrade() {
                    refresh(&w, &s);
                }
            });
        }};
    }
    action!(on_capture, capture);
    action!(on_open, open);
    action!(on_save, |s: &Shared| {
        let _saved = save(s);
    });
    action!(on_export, export);

    // Letter shortcuts return whether they were used, so they are wired by hand.
    let s = Rc::clone(session);
    let w = window.as_weak();
    window.on_shortcut(move |key| {
        let used = s.borrow_mut().shortcut(&key);
        if used && let Some(w) = w.upgrade() {
            refresh(&w, &s);
        }
        used
    });
}

fn wire_close(window: &AppWindow, session: &Shared) {
    // Never lose work on close: ask, and stay open if the user cancels or saving fails.
    let s = Rc::clone(session);
    let w = window.as_weak();
    window.window().on_close_requested(move || {
        if confirm_discard(&s) {
            return slint::CloseRequestResponse::HideWindow;
        }
        if let Some(w) = w.upgrade() {
            refresh(&w, &s);
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

/// Before replacing or closing a dirty document, offers to save. Returns true when it is safe
/// to proceed (saved, discarded on purpose, or nothing to lose).
fn confirm_discard(s: &Shared) -> bool {
    if !s.borrow().is_dirty() {
        return true;
    }
    let answer = rfd::MessageDialog::new()
        .set_title("Unsaved changes")
        .set_description("Save your changes first?")
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
    match answer {
        rfd::MessageDialogResult::No => true,
        rfd::MessageDialogResult::Yes | rfd::MessageDialogResult::Ok => save(s),
        rfd::MessageDialogResult::Cancel | rfd::MessageDialogResult::Custom(_) => false,
    }
}

/// Pushes a snapshot of the session into the window. Never holds a borrow while setting
/// properties, because setting a property can synchronously trigger a UI callback.
pub fn refresh(window: &AppWindow, session: &Rc<RefCell<Session>>) {
    let snapshot = {
        let s = session.borrow();
        Snapshot {
            title: s.title(),
            has_document: s.has_document(),
            canvas: s.render_canvas().map(|img| to_slint_image(&img)),
            tool: match s.tool() {
                Tool::Select => "select".to_owned(),
                Tool::Draw(kind) => TOOLS
                    .iter()
                    .find(|t| t.tool == Tool::Draw(kind))
                    .map_or("select", |t| t.id)
                    .to_owned(),
            },
            swatch: s.swatch(),
            stroke_width: s.stroke_width(),
            undo: s.undo_label().map(str::to_owned),
            redo: s.redo_label().map(str::to_owned),
            status: s.status().to_owned(),
            rows: s.annotation_rows(),
            selected_row: s.selected_row(),
            selection: s.selection_bounds(),
            text: s.selected_text(),
        }
    };
    window.set_window_title(snapshot.title.into());
    window.set_has_document(snapshot.has_document);
    if let Some(canvas) = snapshot.canvas {
        window.set_canvas(canvas);
    }
    window.set_tool(snapshot.tool.into());
    window.set_swatch_index(i32::try_from(snapshot.swatch).unwrap_or(0));
    window.set_stroke_width(snapshot.stroke_width);
    window.set_can_undo(snapshot.undo.is_some());
    window.set_undo_label(snapshot.undo.unwrap_or_default().into());
    window.set_can_redo(snapshot.redo.is_some());
    window.set_redo_label(snapshot.redo.unwrap_or_default().into());
    window.set_status(snapshot.status.into());
    window.set_annotations(ModelRc::new(VecModel::from(
        snapshot
            .rows
            .into_iter()
            .map(|r| slint::StandardListViewItem::from(SharedString::from(r)))
            .collect::<Vec<_>>(),
    )));
    window.set_selected_index(
        snapshot.selected_row.and_then(|r| i32::try_from(r).ok()).unwrap_or(-1),
    );
    window.set_has_selection(snapshot.selection.is_some());
    if let Some(b) = snapshot.selection {
        window.set_sel_x(b.x);
        window.set_sel_y(b.y);
        window.set_sel_w(b.width);
        window.set_sel_h(b.height);
    }
    window.set_selected_is_text(snapshot.text.is_some());
    if let Some(text) = snapshot.text
        && window.get_selected_text() != text.as_str()
    {
        window.set_selected_text(text.into());
    }
}

struct Snapshot {
    title: String,
    has_document: bool,
    canvas: Option<Image>,
    tool: String,
    swatch: usize,
    stroke_width: f32,
    undo: Option<String>,
    redo: Option<String>,
    status: String,
    rows: Vec<String>,
    selected_row: Option<usize>,
    selection: Option<tellshot_core::Rect>,
    text: Option<String>,
}

fn to_slint_image(img: &tellshot_imaging::Image) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        img.as_bytes(),
        img.width(),
        img.height(),
    ))
}

fn icon_for(icons: &Icons<'_>, id: &str) -> SharedString {
    match id {
        "select" => icons.get_select(),
        "arrow" => icons.get_arrow(),
        "line" => icons.get_line(),
        "rectangle" => icons.get_rectangle(),
        "ellipse" => icons.get_ellipse(),
        "text" => icons.get_text(),
        "step" => icons.get_step(),
        "highlight" => icons.get_highlight(),
        "blur" => icons.get_blur(),
        "pixelate" => icons.get_pixelate(),
        "redact" => icons.get_redact(),
        _ => icons.get_pen(),
    }
}

fn capture(s: &Shared) {
    if !confirm_discard(s) {
        return;
    }
    let result =
        tellshot_capture::detect(tellshot_capture::BackendChoice::Auto).and_then(|mut b| {
            let target = b.targets()?.into_iter().next().ok_or_else(|| {
                tellshot_capture::CaptureError::Unsupported("no monitors found".into())
            })?;
            b.capture(&target.id, None)
        });
    match result {
        Ok(frame) => s.borrow_mut().open_image(frame.image, "capture"),
        Err(e) => s.borrow_mut().set_status(format!("Capture failed: {e}")),
    }
}

fn open(s: &Shared) {
    if !confirm_discard(s) {
        return;
    }
    let picked = rfd::FileDialog::new()
        .add_filter("Tellshot project or PNG", &["tellshot", "png"])
        .pick_file();
    let Some(path) = picked else { return };
    let result = s.borrow_mut().open_path(&path);
    if let Err(e) = result {
        s.borrow_mut().set_status(format!("Could not open {}: {e}", path.display()));
    }
}

/// Saves, asking for a location the first time. Returns true when the document was saved.
fn save(s: &Shared) -> bool {
    let known = s.borrow().path().map(Path::to_path_buf);
    let path = if let Some(p) = known {
        p
    } else {
        let picked = rfd::FileDialog::new()
            .add_filter("Tellshot project", &["tellshot"])
            .set_file_name("capture.tellshot")
            .save_file();
        let Some(p) = picked else { return false };
        p
    };
    let result = s.borrow_mut().save_as(&path);
    match result {
        Ok(()) => true,
        Err(e) => {
            s.borrow_mut().set_status(format!("Could not save: {e}"));
            false
        }
    }
}

fn export(s: &Shared) {
    let picked = rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name("capture.png")
        .save_file();
    let Some(path) = picked else { return };
    let result = s.borrow_mut().export_png(&path);
    if let Err(e) = result {
        s.borrow_mut().set_status(format!("Could not export: {e}"));
    }
}
