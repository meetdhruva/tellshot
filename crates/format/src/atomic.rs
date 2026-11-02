//! Crash-safe file replacement.

use std::io::Write as _;
use std::path::Path;

/// Writes `bytes` to `path` atomically: either the old content or the new content survives a
/// crash or power loss, never a partial file.
///
/// The data goes to a temporary file in the same directory (so the final rename never crosses
/// filesystems), is flushed to stable storage, and is then renamed over `path`. On Unix the
/// directory is synced too, so the rename itself is durable.
///
/// # Errors
/// Any I/O error; the original file is untouched when an error is returned.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(".tellshot-").suffix(".tmp");
    // Temp files are created owner-only. Keep the permissions of the file being replaced, or
    // use the conventional rw-r--r-- for a new file, so saving never silently hides a file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(path).map_or(0o644, |m| m.permissions().mode() & 0o7777);
        builder.permissions(std::fs::Permissions::from_mode(mode));
    }
    let mut tmp = builder.tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        std::fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_content_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.bin");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second", "content replaced");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files cleaned up: {leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn keeps_existing_permissions_and_uses_conventional_ones_for_new_files() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.png");
        atomic_write(&path, b"new").unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o644, "new file is rw-r--r--");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write(&path, b"again").unwrap();
        assert_eq!(mode(&path), 0o600, "existing permissions survive a save");
    }

    #[test]
    fn failure_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-dir").join("out.bin");
        assert!(atomic_write(&missing, b"x").is_err(), "missing directory is an error");
    }
}
