//! The protocol between the always-running agent and its clients (editor, CLI).
//!
//! Frames are a little-endian `u32` length followed by a postcard-encoded message. The first
//! message on a connection must be [`Request::Hello`]; the agent answers [`Response::Welcome`] or
//! [`Response::Error`] with [`ErrorCode::VersionMismatch`].
//!
//! # Compatibility rules
//!
//! postcard encodes enum variants by index, so **variants are only ever appended**, never
//! reordered or removed. The `wire_format_is_frozen` test pins existing encodings and fails if
//! someone breaks this. Changing the meaning of a message requires bumping
//! [`PROTOCOL_VERSION`].

use std::io::{Read, Write};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Bumped on incompatible changes.
pub const PROTOCOL_VERSION: u16 = 1;

/// Largest accepted frame. Messages carry paths and small values, never pixels.
pub const MAX_FRAME_BYTES: u32 = 1 << 20;

/// A capture region in physical desktop pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// Client → agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Must be the first message.
    Hello {
        /// The client's [`PROTOCOL_VERSION`].
        protocol: u16,
        /// Client name and version, for logs.
        client: String,
    },
    /// Liveness check.
    Ping,
    /// Capture now and save the result to the library.
    Capture {
        /// Target id from the capture backend; `None` for the primary monitor.
        target: Option<String>,
        /// Area to capture; `None` for the whole target.
        region: Option<Region>,
    },
    /// Open a file in the editor.
    Open {
        /// Path to a `.tellshot` project or an image.
        path: PathBuf,
    },
    /// Stop the agent.
    Shutdown,
}

/// Agent → client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// Accepts the connection.
    Welcome {
        /// The agent's [`PROTOCOL_VERSION`].
        protocol: u16,
        /// Agent name and version.
        server: String,
    },
    /// Reply to [`Request::Ping`].
    Pong,
    /// A capture was saved.
    Captured {
        /// Where the `.tellshot` project was written.
        path: PathBuf,
    },
    /// The request succeeded and has no other result.
    Done,
    /// The request failed.
    Error {
        /// Machine-readable reason.
        code: ErrorCode,
        /// Human-readable explanation.
        message: String,
    },
}

/// Machine-readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Client and agent speak different protocol versions.
    VersionMismatch,
    /// The first message was not `Hello`.
    HandshakeRequired,
    /// Capture failed.
    CaptureFailed,
    /// The request is understood but not supported on this platform or build.
    Unsupported,
    /// Anything else.
    Internal,
}

/// Framing and decoding errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IpcError {
    /// The transport failed or closed.
    #[error("connection error: {0}")]
    Io(#[from] std::io::Error),
    /// A frame exceeded [`MAX_FRAME_BYTES`].
    #[error("message of {0} bytes exceeds the limit")]
    TooLarge(u64),
    /// The payload could not be encoded or decoded.
    #[error("malformed message: {0}")]
    Codec(#[from] postcard::Error),
}

/// Writes one framed message.
///
/// # Errors
/// [`IpcError`] on I/O failure or if the message is too large.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> Result<(), IpcError> {
    let payload = postcard::to_allocvec(message)?;
    let len = u32::try_from(payload.len()).map_err(|_| IpcError::TooLarge(payload.len() as u64))?;
    if len > MAX_FRAME_BYTES {
        return Err(IpcError::TooLarge(u64::from(len)));
    }
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Reads one framed message. Never allocates more than [`MAX_FRAME_BYTES`].
///
/// # Errors
/// [`IpcError`] on I/O failure, oversized frames or malformed payloads.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, IpcError> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len);
    if len > MAX_FRAME_BYTES {
        return Err(IpcError::TooLarge(u64::from(len)));
    }
    let mut payload = vec![0_u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok(postcard::from_bytes(&payload)?)
}

/// Where the agent listens on Unix.
///
/// `$XDG_RUNTIME_DIR/tellshot/agent.sock` is per-user and private. The fallback under the shared
/// temp directory is *not* private by itself: the agent must create it with mode 0700 and refuse
/// to use it if another user owns it.
#[cfg(unix)]
#[must_use]
pub fn socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("tellshot").join("agent.sock");
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    std::env::temp_dir().join(format!("tellshot-{user}")).join("agent.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(m: &T) {
        let mut buf = Vec::new();
        write_message(&mut buf, m).unwrap();
        let back: T = read_message(&mut buf.as_slice()).unwrap();
        assert_eq!(&back, m, "round trip");
    }

    #[test]
    fn messages_round_trip() {
        round_trip(&Request::Hello { protocol: PROTOCOL_VERSION, client: "cli 0.1".into() });
        round_trip(&Request::Capture {
            target: Some("x11:DP-1".into()),
            region: Some(Region { x: -10, y: 5, width: 100, height: 50 }),
        });
        round_trip(&Response::Error { code: ErrorCode::CaptureFailed, message: "denied".into() });
    }

    #[test]
    fn wire_format_is_frozen() {
        // If this fails, a variant was reordered or removed: append new variants instead.
        assert_eq!(postcard::to_allocvec(&Request::Ping).unwrap(), [1], "Request::Ping");
        assert_eq!(postcard::to_allocvec(&Request::Shutdown).unwrap(), [4], "Request::Shutdown");
        assert_eq!(postcard::to_allocvec(&Response::Pong).unwrap(), [1], "Response::Pong");
        assert_eq!(postcard::to_allocvec(&Response::Done).unwrap(), [3], "Response::Done");
        assert_eq!(
            postcard::to_allocvec(&Response::Error {
                code: ErrorCode::Internal,
                message: String::new()
            })
            .unwrap(),
            [4, 4, 0],
            "Response::Error"
        );
    }

    #[test]
    fn oversized_frames_are_rejected_before_allocating() {
        let mut frame = (MAX_FRAME_BYTES + 1).to_le_bytes().to_vec();
        frame.extend_from_slice(&[0; 8]);
        let result: Result<Request, _> = read_message(&mut frame.as_slice());
        assert!(matches!(result, Err(IpcError::TooLarge(_))), "{result:?}");
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..64)) {
            let _decoded: Result<Request, _> = read_message(&mut bytes.as_slice());
        }
    }
}
