//! `tellshot-editor [FILE]`: open the editor, optionally with a `.tellshot` project or PNG.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    match tellshot_editor::launch(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ignored =
                writeln!(std::io::stderr(), "tellshot-editor: could not open a window: {e}");
            ExitCode::FAILURE
        }
    }
}
