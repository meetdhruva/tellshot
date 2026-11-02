//! Everything Tellshot writes to or reads from disk.
//!
//! * [`atomic_write`]: the only way user data is written. Write to a temporary file in the same
//!   directory, flush it to disk, then rename over the target, so a crash or power loss leaves
//!   either the old file or the new one, never a truncated mix.
//! * [`png_codec`]: PNG encode and decode with size limits.
//! * [`project`]: the `.tellshot` project file (a zip with a manifest, the document as JSON and
//!   the untouched base image), versioned and validated on load.

mod atomic;
pub mod png_codec;
pub mod project;

pub use atomic::atomic_write;
pub use project::{FORMAT_VERSION, FormatVersion, Project};

/// Errors from reading or writing Tellshot files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FormatError {
    /// An operating-system I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The PNG could not be decoded or encoded.
    #[error("PNG error: {0}")]
    Png(String),
    /// The zip container is damaged or missing an entry.
    #[error("damaged project file: {0}")]
    Container(String),
    /// JSON in the project is malformed.
    #[error("malformed project data: {0}")]
    Json(#[from] serde_json::Error),
    /// The file was written by a newer, incompatible Tellshot.
    #[error("this file needs a newer Tellshot (file format {found}, this build reads {supported})")]
    UnsupportedVersion {
        /// Version found in the file.
        found: FormatVersion,
        /// Newest version this build reads.
        supported: FormatVersion,
    },
    /// The data is well-formed but violates an invariant or a size limit.
    #[error("invalid project: {0}")]
    Invalid(String),
}

/// Shorthand result type.
pub type Result<T, E = FormatError> = std::result::Result<T, E>;
