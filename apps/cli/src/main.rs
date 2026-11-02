//! `tellshot`: capture, annotate and export screenshots from the terminal, scripts and AI agents.
//!
//! Every command accepts `--json` for machine-readable output. Errors carry a stable `code` and
//! exit status (see `error.rs`), so scripts can branch without parsing prose.

mod error;
mod mcp;
mod ops;

use std::fmt::Write as _;
use std::io::{Read as _, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tellshot_capture::{BackendChoice, PhysicalRect};
use tellshot_core::Edit;

use crate::error::{CliError, ErrorKind};
use crate::ops::SchemaKind;

/// Capture, annotate and export screenshots.
#[derive(Debug, Parser)]
#[command(name = "tellshot", version, about, long_about = None, propagate_version = true)]
struct Cli {
    /// Print machine-readable JSON instead of text (also for errors).
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Backend {
    /// Best native backend for this session.
    Auto,
    /// Deterministic test pattern; works anywhere, including headless CI and sandboxes.
    Synthetic,
}

impl From<Backend> for BackendChoice {
    fn from(b: Backend) -> Self {
        match b {
            Backend::Auto => Self::Auto,
            Backend::Synthetic => Self::Synthetic,
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List monitors that can be captured.
    Targets {
        #[arg(long, value_enum, default_value_t = Backend::Auto)]
        backend: Backend,
    },
    /// Capture a monitor or region to a .tellshot project or .png.
    Capture {
        /// Output path (.tellshot keeps everything editable; .png is a flat image).
        #[arg(short, long)]
        output: PathBuf,
        /// Target id from `tellshot targets` (default: primary monitor).
        #[arg(long)]
        target: Option<String>,
        /// Region as X,Y,WIDTH,HEIGHT in physical desktop pixels.
        #[arg(long, value_parser = parse_region)]
        region: Option<PhysicalRect>,
        #[arg(long, value_enum, default_value_t = Backend::Auto)]
        backend: Backend,
        /// Replace the output if it exists.
        #[arg(long)]
        overwrite: bool,
    },
    /// Create a .tellshot project from a PNG.
    New {
        /// Source image.
        image: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// List the annotations in a project.
    Describe {
        /// A .tellshot project.
        project: PathBuf,
    },
    /// Apply edits (JSON) to a project, all or nothing. See `tellshot schema edit`.
    Edit {
        /// A .tellshot project.
        project: PathBuf,
        /// One edit or an array of edits as JSON; `-` reads from stdin.
        #[arg(long, conflicts_with = "edits_file")]
        edit: Option<String>,
        /// A file containing a JSON array of edits.
        #[arg(long)]
        edits_file: Option<PathBuf>,
    },
    /// Render a project to a PNG with annotations flattened (redactions destroy pixels).
    Export {
        project: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Stitch overlapping frames of a scrolling page into one tall image.
    Stitch {
        /// Frames in scroll order (PNG).
        #[arg(required = true, num_args = 1..)]
        frames: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Print the JSON Schema for edits or documents.
    Schema {
        #[arg(value_enum)]
        which: SchemaKind,
    },
    /// Run a Model Context Protocol server on stdio for AI agents.
    Mcp {
        /// Only allow reading and writing files inside this folder (recommended).
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn parse_region(spec: &str) -> Result<PhysicalRect, String> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let [left, top, width, height] = parts.as_slice() else {
        return Err("expected X,Y,WIDTH,HEIGHT, for example 0,0,800,600".into());
    };
    let coord = |v: &str| v.parse::<i32>().map_err(|e| format!("{v:?}: {e}"));
    let extent = |v: &str| {
        v.parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("{v:?} must be a positive integer"))
    };
    Ok(PhysicalRect {
        x: coord(left)?,
        y: coord(top)?,
        width: extent(width)?,
        height: extent(height)?,
    })
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("TELLSHOT_LOG"))
        .init();
    let json = cli.json;
    match run(cli.command, json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report(&e, json);
            ExitCode::from(e.code.exit_code())
        }
    }
}

fn run(command: Command, json: bool) -> Result<(), CliError> {
    match command {
        Command::Targets { backend } => {
            let list = ops::targets(backend.into())?;
            emit(json, &list, |text| {
                writeln!(text, "backend: {}", list.backend)?;
                for t in &list.targets {
                    let b = t.bounds;
                    let primary = if t.primary { " (primary)" } else { "" };
                    writeln!(
                        text,
                        "{}  {}{primary}  {}×{} at {},{}  scale {}",
                        t.id, t.name, b.width, b.height, b.x, b.y, t.scale_factor
                    )?;
                }
                Ok(())
            })
        }
        Command::Capture { output, target, region, backend, overwrite } => {
            let c = ops::capture(ops::CaptureRequest {
                backend: backend.into(),
                target,
                region,
                output,
                overwrite,
            })?;
            emit(json, &c, |text| {
                let s = &c.saved;
                writeln!(
                    text,
                    "saved {} ({}×{}) from {} via {}",
                    s.path.display(),
                    s.width,
                    s.height,
                    c.target,
                    c.backend
                )
            })
        }
        Command::New { image, output, overwrite } => {
            saved(json, "created", &ops::new_project(&image, &output, overwrite)?)
        }
        Command::Describe { project } => {
            let d = ops::describe(&project)?;
            emit(json, &d, |text| {
                writeln!(
                    text,
                    "{}×{} image, {} annotation(s)",
                    d.width,
                    d.height,
                    d.objects.len()
                )?;
                d.objects.iter().try_for_each(|o| writeln!(text, "  {}", o.description))
            })
        }
        Command::Edit { project, edit, edits_file } => {
            let summary = ops::apply_edits(&project, read_edits(edit, edits_file)?)?;
            emit(json, &summary, |text| {
                for a in &summary.applied {
                    match a.created {
                        Some(id) => writeln!(text, "{} → {id}", a.label)?,
                        None => writeln!(text, "{}", a.label)?,
                    }
                }
                writeln!(text, "saved {} ({} objects)", summary.path.display(), summary.objects)
            })
        }
        Command::Export { project, output, overwrite } => {
            saved(json, "exported", &ops::export(&project, &output, overwrite)?)
        }
        Command::Stitch { frames, output, overwrite } => {
            saved(json, "stitched", &ops::stitch(&frames, &output, overwrite)?)
        }
        Command::Schema { which } => write_stdout(&format!("{:#}\n", ops::schema(which))),
        Command::Mcp { root } => {
            let sandbox = mcp::Sandbox::new(root.as_deref())?;
            mcp::serve(std::io::stdin().lock(), std::io::stdout().lock(), &sandbox)
                .map_err(CliError::from)
        }
    }
}

fn saved(json: bool, verb: &str, s: &ops::Saved) -> Result<(), CliError> {
    emit(json, s, |text| writeln!(text, "{verb} {} ({}×{})", s.path.display(), s.width, s.height))
}

/// Reads edits from `--edit '<json>'`, `--edit -` (stdin) or `--edits-file`.
fn read_edits(edit: Option<String>, edits_file: Option<PathBuf>) -> Result<Vec<Edit>, CliError> {
    let raw = match (edit, edits_file) {
        (Some(s), _) if s == "-" => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
        (Some(s), _) => s,
        (None, Some(path)) => std::fs::read_to_string(path)?,
        (None, None) => {
            return Err(CliError::new(ErrorKind::InvalidInput, "nothing to apply")
                .with_hint("pass --edit '<json>' or --edits-file <path>"));
        }
    };
    parse_edits(&raw)
}

/// Accepts a single edit object or an array of edits.
fn parse_edits(raw: &str) -> Result<Vec<Edit>, CliError> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    if value.is_array() {
        Ok(serde_json::from_value(value)?)
    } else {
        Ok(vec![serde_json::from_value(value)?])
    }
}

/// Prints `value` as JSON, or the human-readable text written by `human`.
fn emit<T: Serialize>(
    json: bool,
    value: &T,
    human: impl FnOnce(&mut String) -> std::fmt::Result,
) -> Result<(), CliError> {
    if json {
        return write_stdout(&format!("{}\n", serde_json::to_string_pretty(value)?));
    }
    let mut text = String::new();
    human(&mut text)?;
    write_stdout(&text)
}

/// Writes to stdout; a closed pipe (`tellshot targets | head -1`) is not an error.
fn write_stdout(text: &str) -> Result<(), CliError> {
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(CliError::from),
    }
}

fn report(e: &CliError, json: bool) {
    if json {
        let body = serde_json::json!({ "error": e });
        let _ignored = writeln!(std::io::stdout().lock(), "{body:#}");
    } else {
        let _ignored = writeln!(std::io::stderr().lock(), "error: {e}");
    }
}
