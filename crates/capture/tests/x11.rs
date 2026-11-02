//! Real X11 capture, run under Xvfb in CI (`xvfb-run cargo test`). Skipped without a display.
#![cfg(all(unix, not(target_os = "macos")))]

use tellshot_capture::{BackendChoice, PhysicalRect, detect};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, CreateGCAux, Rectangle, SubwindowMode};

#[test]
fn captures_what_was_drawn_on_the_root_window() {
    if std::env::var_os("DISPLAY").is_none() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return;
    }
    // Paint a known colour onto the root window.
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let root = conn.setup().roots[screen_num].root;
    let gc = conn.generate_id().unwrap();
    let aux =
        CreateGCAux::new().foreground(0x00d5_5e00).subwindow_mode(SubwindowMode::INCLUDE_INFERIORS);
    conn.create_gc(gc, root, &aux).unwrap();
    conn.poly_fill_rectangle(root, gc, &[Rectangle { x: 10, y: 20, width: 40, height: 30 }])
        .unwrap();
    // A round trip guarantees the server has drawn before we capture.
    conn.get_input_focus().unwrap().reply().unwrap();

    let mut backend = detect(BackendChoice::Auto).unwrap();
    assert!(backend.name().starts_with("x11"), "backend {}", backend.name());
    let targets = backend.targets().unwrap();
    let primary = targets.first().unwrap();
    let region = PhysicalRect { x: 0, y: 0, width: 64, height: 64 };
    let frame = backend.capture(&primary.id, Some(region)).unwrap();
    assert_eq!((frame.image.width(), frame.image.height()), (64, 64), "region size");
    assert_eq!(frame.image.pixel(20, 30), Some([0xd5, 0x5e, 0x00, 255]), "painted pixel");
    assert_eq!(frame.image.pixel(5, 5), Some([0, 0, 0, 255]), "unpainted pixel");
}
