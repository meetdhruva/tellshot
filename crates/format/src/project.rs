//! The `.tellshot` project file.
//!
//! A zip archive with three entries:
//!
//! | entry           | content                                              |
//! |-----------------|------------------------------------------------------|
//! | `manifest.json` | format name and version, the app that wrote it        |
//! | `document.json` | the [`Document`] (annotations, crop), human-readable  |
//! | `base.png`      | the original capture, never modified                 |
//!
//! JSON keeps projects diffable and easy for scripts and AI agents to inspect. The base image is
//! stored losslessly and untouched, so every edit stays reversible.
//!
//! # Versioning
//!
//! `major` changes when existing readers would misread a file; this build refuses newer majors
//! with a clear message instead of guessing. `minor` changes may only add new zip entries, which
//! older readers ignore. See ADR 0005.

use std::fmt;
use std::io::{Cursor, Read, Write as _};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tellshot_core::{BaseImage, Document};
use tellshot_imaging::Image;

use crate::png_codec::{self, Effort};
use crate::{FormatError, Result, atomic_write};

/// A `major.minor` file format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FormatVersion {
    /// Incompatible changes.
    pub major: u16,
    /// Backwards-compatible additions.
    pub minor: u16,
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The version this build writes and the newest it reads.
pub const FORMAT_VERSION: FormatVersion = FormatVersion { major: 1, minor: 0 };

const MAGIC: &str = "tellshot";
const MANIFEST: &str = "manifest.json";
const DOCUMENT: &str = "document.json";
const BASE: &str = "base.png";
const MAX_JSON_BYTES: u64 = 64 << 20;
/// Largest raw image (2 GiB) plus generous room for PNG filter bytes and zlib framing on
/// incompressible content, so every image we accept can also be saved and reopened.
const MAX_PNG_BYTES: u64 = 3 << 30;

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: FormatVersion,
    generator: String,
}

/// A document together with its base image.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// Annotations and crop.
    pub document: Document,
    /// The original capture.
    pub base: Image,
}

impl Project {
    /// A new project with no annotations.
    #[must_use]
    pub fn new(base: Image) -> Self {
        let document = Document::new(BaseImage { width: base.width(), height: base.height() });
        Self { document, base }
    }

    /// Serialises to `.tellshot` bytes, after checking the document is valid.
    ///
    /// # Errors
    /// When the document is invalid or encoding fails.
    pub fn to_bytes(&self, generator: &str) -> Result<Vec<u8>> {
        // Never write a file that this build would refuse to open.
        self.document.validate().map_err(|e| FormatError::Invalid(e.to_string()))?;
        let manifest =
            Manifest { format: MAGIC.into(), version: FORMAT_VERSION, generator: generator.into() };
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        // PNG is already compressed; deflating it again wastes time for nothing.
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(self.base.as_bytes().len() > (1 << 31));
        let container = |e: zip::result::ZipError| FormatError::Container(e.to_string());

        zip.start_file(MANIFEST, deflated).map_err(container)?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
        zip.start_file(DOCUMENT, deflated).map_err(container)?;
        zip.write_all(&serde_json::to_vec_pretty(&self.document)?)?;
        zip.start_file(BASE, stored).map_err(container)?;
        zip.write_all(&png_codec::encode(&self.base, Effort::Fast)?)?;
        Ok(zip.finish().map_err(container)?.into_inner())
    }

    /// Parses `.tellshot` bytes, checking the version, limits and every document invariant.
    ///
    /// # Errors
    /// See [`FormatError`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| FormatError::Container(e.to_string()))?;

        let manifest: Manifest =
            serde_json::from_slice(&read_entry(&mut zip, MANIFEST, MAX_JSON_BYTES)?)?;
        if manifest.format != MAGIC {
            return Err(FormatError::Container(format!(
                "not a Tellshot file (format {:?})",
                manifest.format
            )));
        }
        if manifest.version.major > FORMAT_VERSION.major {
            return Err(FormatError::UnsupportedVersion {
                found: manifest.version,
                supported: FORMAT_VERSION,
            });
        }

        let document: Document =
            serde_json::from_slice(&read_entry(&mut zip, DOCUMENT, MAX_JSON_BYTES)?)?;
        document.validate().map_err(|e| FormatError::Invalid(e.to_string()))?;
        let base = png_codec::decode(&read_entry(&mut zip, BASE, MAX_PNG_BYTES)?)?;
        let expected = document.base();
        if (base.width(), base.height()) != (expected.width, expected.height) {
            return Err(FormatError::Invalid(format!(
                "base image is {}×{} but the document expects {}×{}",
                base.width(),
                base.height(),
                expected.width,
                expected.height
            )));
        }
        Ok(Self { document, base })
    }

    /// Saves atomically to `path`.
    ///
    /// # Errors
    /// See [`FormatError`].
    pub fn save(&self, path: &Path, generator: &str) -> Result<()> {
        atomic_write(path, &self.to_bytes(generator)?)?;
        Ok(())
    }

    /// Loads from `path`.
    ///
    /// # Errors
    /// See [`FormatError`].
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_bytes(&std::fs::read(path)?)
    }
}

/// Reads one zip entry, refusing anything that would decompress beyond `limit` (zip bombs).
fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, limit: u64) -> Result<Vec<u8>> {
    let entry = zip.by_name(name).map_err(|_| FormatError::Container(format!("missing {name}")))?;
    if entry.size() > limit {
        return Err(FormatError::Invalid(format!("{name} is too large")));
    }
    let mut out = Vec::new();
    entry.take(limit + 1).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(FormatError::Invalid(format!("{name} is too large")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use tellshot_core::{Edit, History, Point, Shape};

    use super::*;

    fn sample() -> Project {
        let base = Image::new(32, 24, [200, 210, 220, 255]).unwrap();
        let mut project = Project::new(base);
        let mut history = History::new();
        let arrow = Shape::Arrow { from: Point::new(1.0, 1.0), to: Point::new(20.0, 20.0) };
        history
            .apply(&mut project.document, Edit::Add { shape: arrow, style: None }, None)
            .unwrap();
        project
    }

    #[test]
    fn round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.tellshot");
        let project = sample();
        project.save(&path, "tellshot-test").unwrap();
        assert_eq!(Project::load(&path).unwrap(), project, "identical after reload");
    }

    #[test]
    fn newer_major_versions_are_refused_with_a_clear_error() {
        let bytes = sample().to_bytes("test").unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).unwrap();
            let name = entry.name().unwrap().into_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == MANIFEST {
                data = br#"{"format":"tellshot","version":{"major":9,"minor":0},"generator":"future"}"#.to_vec();
            }
            out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            out.write_all(&data).unwrap();
        }
        let future = out.finish().unwrap().into_inner();
        let err = Project::from_bytes(&future).unwrap_err();
        assert!(matches!(err, FormatError::UnsupportedVersion { .. }), "{err}");
        assert!(err.to_string().contains("newer Tellshot"), "message explains what to do: {err}");
    }

    #[test]
    fn damaged_files_fail_cleanly() {
        assert!(Project::from_bytes(b"").is_err(), "empty");
        assert!(Project::from_bytes(b"PK\x03\x04garbage").is_err(), "truncated zip");
        let mut bytes = sample().to_bytes("test").unwrap();
        let mid = bytes.len() / 2;
        bytes.truncate(mid);
        assert!(Project::from_bytes(&bytes).is_err(), "half a file");
    }
}
