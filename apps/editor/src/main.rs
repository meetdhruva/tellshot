//! `tellshot-editor [FILE]`: open the editor, optionally with a `.tellshot` project or PNG.

// Release builds are GUI apps on Windows: no console window behind the editor.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let first = std::env::args_os().nth(1);
    match first.as_ref().and_then(|a| a.to_str()) {
        Some("--version" | "-V") => {
            let _ignored =
                writeln!(std::io::stdout(), "tellshot-editor {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("--help" | "-h") => {
            let _ignored = writeln!(
                std::io::stdout(),
                "Usage: tellshot-editor [FILE]\n\nOpens the Tellshot editor, optionally with a \
                 .tellshot project or a PNG image."
            );
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    match tellshot_editor::launch(first.map(std::path::PathBuf::from)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let message = format!("Tellshot could not open a window: {e}");
            let _ignored = writeln!(std::io::stderr(), "tellshot-editor: {message}");
            // A GUI app on Windows has no visible stderr, so also show a dialog.
            let _shown = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Tellshot")
                .set_description(message)
                .show();
            ExitCode::FAILURE
        }
    }
}
