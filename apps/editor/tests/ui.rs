//! UI tests through the accessibility tree: the same names and actions a screen reader (or an AI
//! agent using Slint's MCP server) relies on. If a control loses its accessible name, these fail.
//!
//! Slint's testing platform can be installed once per process, so this is a single test that
//! walks through several scenarios in order.
#![expect(clippy::panic, reason = "test helper: a missing control should fail loudly")]

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, Model as _, SharedString};
use tellshot_core::ShapeKind;
use tellshot_editor::session::Tool;
use tellshot_editor::{AppWindow, attach, refresh};
use tellshot_imaging::Image;
use tellshot_render::Fonts;

fn press(window: &AppWindow, text: impl Into<SharedString>) {
    let text = text.into();
    window.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.window().dispatch_event(WindowEvent::KeyReleased { text });
}

fn button(window: &AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(window, label)
        .next()
        .unwrap_or_else(|| panic!("no control named {label:?}"))
}

/// Every interactive control has a non-empty accessible name (custom and std widgets alike).
fn assert_every_control_is_named(window: &AppWindow) {
    let controls = ElementQuery::from_root(window)
        .match_descendants()
        .match_predicate(|e| {
            e.accessible_role().is_some_and(|r| {
                matches!(
                    r,
                    AccessibleRole::Button
                        | AccessibleRole::Checkbox
                        | AccessibleRole::Slider
                        | AccessibleRole::List
                        | AccessibleRole::TextInput
                        | AccessibleRole::Switch
                        | AccessibleRole::Combobox
                )
            })
        })
        .find_all();
    for c in &controls {
        let label = c.accessible_label().unwrap_or_default();
        assert!(!label.trim().is_empty(), "a {:?} has no accessible name", c.accessible_role());
    }
}

#[test]
fn editor_is_fully_operable_through_accessibility_and_keyboard() {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap();
    let session = attach(&window, Fonts::none());
    window.show().unwrap();

    // Empty state: tools exist but are disabled until something is open.
    let rectangle = button(&window, "Rectangle");
    assert_eq!(rectangle.accessible_enabled(), Some(false), "tools disabled without a document");
    let _capture = button(&window, "Capture screen");

    session.borrow_mut().open_image(Image::new(400, 300, [240, 240, 240, 255]).unwrap(), "test");
    refresh(&window, &session);

    assert_every_control_is_named(&window);
    let buttons = ElementQuery::from_root(&window)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Button)
        .find_all();
    assert!(
        buttons.len() >= 20,
        "toolbar, swatches and actions are buttons, found {}",
        buttons.len()
    );
    for b in &buttons {
        let label = b.accessible_label().unwrap_or_default();
        assert!(!label.trim().is_empty(), "a button has no accessible name: {:?}", b.id());
    }

    // Activating a tool through accessibility selects it and reports it as checked.
    button(&window, "Rectangle").invoke_accessible_default_action();
    assert_eq!(session.borrow().tool(), Tool::Draw(ShapeKind::Rectangle), "rectangle tool");
    assert_eq!(
        button(&window, "Rectangle").accessible_checked(),
        Some(true),
        "reported as checked"
    );
    assert_eq!(button(&window, "Arrow").accessible_checked(), Some(false), "others unchecked");

    // Keyboard: letter shortcut, Enter inserts, arrows nudge, Delete removes, Ctrl+Z undoes.
    press(&window, "a");
    assert_eq!(session.borrow().tool(), Tool::Draw(ShapeKind::Arrow), "A selects the arrow tool");
    press(&window, Key::Return);
    assert_eq!(window.get_annotations().row_count(), 1, "Enter inserted an arrow");
    assert!(window.get_has_selection(), "the new arrow is selected");
    press(&window, Key::RightArrow);
    assert_eq!(window.get_undo_label(), "Move arrow", "nudge is undoable");

    // The undo button's accessible name says what it will undo.
    button(&window, "Undo Move arrow").invoke_accessible_default_action();
    button(&window, "Undo Add arrow").invoke_accessible_default_action();
    assert_eq!(window.get_annotations().row_count(), 0, "undone through accessibility actions");

    // Colour swatches are named, not just coloured.
    button(&window, "Bluish green").invoke_accessible_default_action();
    assert_eq!(session.borrow().swatch(), 3, "swatch chosen by name");
    assert_eq!(button(&window, "Bluish green").accessible_checked(), Some(true), "swatch checked");

    // The annotations list mirrors the canvas for screen-reader users.
    press(&window, Key::Return);
    let rows: Vec<String> = window.get_annotations().iter().map(|r| r.text.to_string()).collect();
    assert_eq!(rows.len(), 1, "one annotation");
    assert!(rows[0].starts_with("Arrow #2"), "ids are never reused: {rows:?}");
    press(&window, Key::Delete);
    assert_eq!(window.get_annotations().row_count(), 0, "Delete removes the selection");

    assert!(window.get_window_title().contains('•'), "unsaved marker in the title");
}
