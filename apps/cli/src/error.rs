//! One error type for every command, with stable machine-readable codes and exit codes.

use std::fmt;

use serde::Serialize;

/// Stable error categories. Codes and exit statuses are part of the CLI's public contract:
/// scripts and agents branch on them, so never renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorKind {
    /// Unexpected failure; a bug or an environment problem.
    Internal,
    /// Bad arguments or malformed input (JSON, edits, files).
    InvalidInput,
    /// A file, target or object does not exist.
    NotFound,
    /// The output exists and `--overwrite` was not given.
    AlreadyExists,
    /// Capture is not possible here (Wayland, permissions, no display).
    CaptureUnavailable,
    /// Reading or writing a file failed.
    Io,
}

impl ErrorKind {
    /// Process exit status for this kind (2 is reserved for usage errors from clap).
    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::Internal => 1,
            Self::InvalidInput => 3,
            Self::NotFound => 4,
            Self::AlreadyExists => 5,
            Self::CaptureUnavailable => 6,
            Self::Io => 7,
        }
    }
}

/// An error with an explanation and, where possible, what to do about it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CliError {
    pub(crate) code: ErrorKind,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hint: Option<String>,
}

impl CliError {
    pub(crate) fn new(code: ErrorKind, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), hint: None }
    }

    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\nhint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CliError {}

impl From<tellshot_format::FormatError> for CliError {
    fn from(e: tellshot_format::FormatError) -> Self {
        use tellshot_format::FormatError as F;
        let kind = match &e {
            F::Io(io) if io.kind() == std::io::ErrorKind::NotFound => ErrorKind::NotFound,
            F::Io(_) => ErrorKind::Io,
            // FormatError is non_exhaustive: anything new is treated as bad input.
            F::Png(_)
            | F::Container(_)
            | F::Json(_)
            | F::UnsupportedVersion { .. }
            | F::Invalid(_)
            | _ => ErrorKind::InvalidInput,
        };
        Self::new(kind, e.to_string())
    }
}

impl From<tellshot_capture::CaptureError> for CliError {
    fn from(e: tellshot_capture::CaptureError) -> Self {
        use tellshot_capture::CaptureError as C;
        match &e {
            C::NotFound(_) => Self::new(ErrorKind::NotFound, e.to_string())
                .with_hint("run `tellshot targets` to list what can be captured"),
            C::EmptyRegion => Self::new(ErrorKind::InvalidInput, e.to_string()),
            C::Unsupported(_) | C::PermissionDenied(_) => {
                Self::new(ErrorKind::CaptureUnavailable, e.to_string())
                    .with_hint("`--backend synthetic` captures a test pattern anywhere")
            }
            // CaptureError is non_exhaustive: anything new is an internal failure.
            C::Backend(_) | _ => Self::new(ErrorKind::Internal, e.to_string()),
        }
    }
}

impl From<tellshot_render::RenderError> for CliError {
    fn from(e: tellshot_render::RenderError) -> Self {
        Self::new(ErrorKind::InvalidInput, e.to_string())
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        let kind = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorKind::NotFound
        } else {
            ErrorKind::Io
        };
        Self::new(kind, e.to_string())
    }
}

impl From<fmt::Error> for CliError {
    fn from(e: fmt::Error) -> Self {
        Self::new(ErrorKind::Internal, e.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(ErrorKind::InvalidInput, format!("invalid JSON: {e}"))
            .with_hint("run `tellshot schema edit` for the exact format")
    }
}
