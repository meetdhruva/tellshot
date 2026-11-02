//! The operations behind every command and MCP tool. One implementation, so the terminal,
//! scripts and AI agents always get identical behaviour.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tellshot_capture::{BackendChoice, PhysicalRect, Target, TargetId};
use tellshot_core::{Edit, History, ObjectId, Rect};
use tellshot_format::png_codec::{self, Effort};
use tellshot_format::{Project, atomic_write};
use tellshot_imaging::scroll::{ScrollOptions, Stitcher};
use tellshot_imaging::{Image, effects};
use tellshot_render::Fonts;

use crate::error::{CliError, ErrorKind};

/// Written into every project's manifest.
pub(crate) const GENERATOR: &str = concat!("tellshot-cli ", env!("CARGO_PKG_VERSION"));

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(Fonts::system)
}

/// Where output may be written, checked before doing any work.
fn check_output(path: &Path, allowed: &[&str], overwrite: bool) -> Result<(), CliError> {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !ext.as_deref().is_some_and(|e| allowed.contains(&e)) {
        return Err(CliError::new(
            ErrorKind::InvalidInput,
            format!("{} must end in .{}", path.display(), allowed.join(" or .")),
        ));
    }
    if !overwrite && path.exists() {
        return Err(CliError::new(
            ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        )
        .with_hint("pass --overwrite (or \"overwrite\": true) to replace it"));
    }
    Ok(())
}

fn is_png(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("png"))
}

/// Opens a `.tellshot` project, or wraps a PNG as a new project.
pub(crate) fn open(path: &Path) -> Result<Project, CliError> {
    if is_png(path) {
        let image = png_codec::decode(&std::fs::read(path)?)?;
        return Ok(Project::new(image));
    }
    Ok(Project::load(path)?)
}

#[derive(Debug, Serialize)]
pub(crate) struct TargetList {
    pub(crate) backend: String,
    pub(crate) targets: Vec<Target>,
}

pub(crate) fn targets(backend: BackendChoice) -> Result<TargetList, CliError> {
    let mut b = tellshot_capture::detect(backend)?;
    Ok(TargetList { backend: b.name().to_owned(), targets: b.targets()? })
}

#[derive(Debug)]
pub(crate) struct CaptureRequest {
    pub(crate) backend: BackendChoice,
    pub(crate) target: Option<String>,
    pub(crate) region: Option<PhysicalRect>,
    pub(crate) output: PathBuf,
    pub(crate) overwrite: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Saved {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct Captured {
    #[serde(flatten)]
    pub(crate) saved: Saved,
    pub(crate) backend: String,
    pub(crate) target: TargetId,
    pub(crate) bounds: PhysicalRect,
    pub(crate) scale_factor: f32,
}

pub(crate) fn capture(req: CaptureRequest) -> Result<Captured, CliError> {
    check_output(&req.output, &["tellshot", "png"], req.overwrite)?;
    let mut backend = tellshot_capture::detect(req.backend)?;
    let target = match req.target {
        Some(id) => TargetId(id),
        None => backend
            .targets()?
            .into_iter()
            .next()
            .map(|t| t.id)
            .ok_or_else(|| CliError::new(ErrorKind::CaptureUnavailable, "no monitors found"))?,
    };
    let frame = backend.capture(&target, req.region)?;
    let (width, height) = (frame.image.width(), frame.image.height());
    // Save first: whatever happens next, the capture is never lost.
    if is_png(&req.output) {
        atomic_write(&req.output, &png_codec::encode(&frame.image, Effort::Fast)?)?;
    } else {
        Project::new(frame.image).save(&req.output, GENERATOR)?;
    }
    Ok(Captured {
        saved: Saved { path: req.output, width, height },
        backend: backend.name().to_owned(),
        target: frame.target,
        bounds: frame.bounds,
        scale_factor: frame.scale_factor,
    })
}

pub(crate) fn new_project(image: &Path, output: &Path, overwrite: bool) -> Result<Saved, CliError> {
    check_output(output, &["tellshot"], overwrite)?;
    let project = Project::new(png_codec::decode(&std::fs::read(image)?)?);
    project.save(output, GENERATOR)?;
    Ok(Saved {
        path: output.to_owned(),
        width: project.base.width(),
        height: project.base.height(),
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct ObjectInfo {
    pub(crate) id: ObjectId,
    pub(crate) kind: tellshot_core::ShapeKind,
    pub(crate) description: String,
    pub(crate) bounds: Rect,
}

#[derive(Debug, Serialize)]
pub(crate) struct Description {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) crop: Option<Rect>,
    pub(crate) objects: Vec<ObjectInfo>,
}

pub(crate) fn describe(path: &Path) -> Result<Description, CliError> {
    let project = open(path)?;
    let doc = &project.document;
    let objects = doc
        .objects()
        .iter()
        .map(|o| ObjectInfo {
            id: o.id,
            kind: o.shape.kind(),
            description: o.describe(doc.step_number(o.id)),
            bounds: o.shape.bounds(),
        })
        .collect();
    Ok(Description {
        width: doc.base().width,
        height: doc.base().height,
        crop: doc.crop(),
        objects,
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct AppliedEdit {
    pub(crate) label: String,
    /// The id of the object the edit created, for `add`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) created: Option<ObjectId>,
    /// True when the edit was valid but changed nothing (for example a zero move).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) unchanged: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct EditSummary {
    pub(crate) path: PathBuf,
    pub(crate) applied: Vec<AppliedEdit>,
    pub(crate) objects: usize,
}

/// Applies all edits or none: on the first invalid edit nothing is written.
pub(crate) fn apply_edits(path: &Path, edits: Vec<Edit>) -> Result<EditSummary, CliError> {
    let is_project = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tellshot"));
    if !is_project {
        return Err(CliError::new(
            ErrorKind::InvalidInput,
            "edits are stored in .tellshot projects",
        )
        .with_hint("run `tellshot new <image.png> -o <project.tellshot>` first"));
    }
    let mut project = Project::load(path)?;
    let mut history = History::new();
    let mut applied = Vec::with_capacity(edits.len());
    for (i, edit) in edits.into_iter().enumerate() {
        let result = history.apply(&mut project.document, edit, None).map_err(|e| {
            CliError::new(ErrorKind::InvalidInput, format!("edit #{i} rejected: {e}"))
                .with_hint("no edits were saved; fix this one and resend the whole list")
        })?;
        // One entry per input edit, so `applied[i]` always describes `edits[i]`.
        applied.push(match result {
            Some(done) => {
                let created = if let tellshot_core::Op::Insert { object, .. } = &done.op {
                    Some(object.id)
                } else {
                    None
                };
                AppliedEdit { label: done.label, created, unchanged: false }
            }
            None => AppliedEdit { label: "No change".into(), created: None, unchanged: true },
        });
    }
    project.save(path, GENERATOR)?;
    Ok(EditSummary { path: path.to_owned(), applied, objects: project.document.objects().len() })
}

pub(crate) fn render(path: &Path) -> Result<Image, CliError> {
    let project = open(path)?;
    Ok(tellshot_render::render(&project.document, &project.base, fonts())?)
}

pub(crate) fn export(path: &Path, output: &Path, overwrite: bool) -> Result<Saved, CliError> {
    check_output(output, &["png"], overwrite)?;
    let image = render(path)?;
    atomic_write(output, &png_codec::encode(&image, Effort::Small)?)?;
    Ok(Saved { path: output.to_owned(), width: image.width(), height: image.height() })
}

/// A downscaled PNG of the rendered project, for agents that can look at images.
pub(crate) fn preview(path: &Path, max_size: u32) -> Result<(Vec<u8>, u32, u32), CliError> {
    let image = effects::downscale_to_fit(&render(path)?, max_size, max_size);
    Ok((png_codec::encode(&image, Effort::Fast)?, image.width(), image.height()))
}

pub(crate) fn stitch(
    frames: &[PathBuf],
    output: &Path,
    overwrite: bool,
) -> Result<Saved, CliError> {
    check_output(output, &["png", "tellshot"], overwrite)?;
    let mut stitcher = Stitcher::new(ScrollOptions::default());
    for (i, path) in frames.iter().enumerate() {
        let frame = png_codec::decode(&std::fs::read(path)?)?;
        stitcher.push(frame).map_err(|e| {
            CliError::new(
                ErrorKind::InvalidInput,
                format!("frame {} ({}): {e}", i + 1, path.display()),
            )
        })?;
    }
    let image = stitcher
        .finish()
        .ok_or_else(|| CliError::new(ErrorKind::InvalidInput, "no frames given"))?;
    let (width, height) = (image.width(), image.height());
    if is_png(output) {
        atomic_write(output, &png_codec::encode(&image, Effort::Small)?)?;
    } else {
        Project::new(image).save(output, GENERATOR)?;
    }
    Ok(Saved { path: output.to_owned(), width, height })
}

/// JSON Schema for edits or documents.
pub(crate) fn schema(which: SchemaKind) -> serde_json::Value {
    let schema = match which {
        SchemaKind::Edit => schemars::schema_for!(Vec<Edit>),
        SchemaKind::Document => schemars::schema_for!(tellshot_core::Document),
    };
    schema.to_value()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum SchemaKind {
    /// A list of edits for `tellshot edit` and the `apply_edits` MCP tool.
    Edit,
    /// The `document.json` inside a .tellshot project.
    Document,
}
