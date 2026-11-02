//! `tellshot mcp`: a Model Context Protocol server over stdio, so AI agents can capture, annotate
//! and export screenshots with the same operations as the CLI.
//!
//! Newline-delimited JSON-RPC 2.0. Stdout carries only protocol messages; logs go to stderr.
//! Tools that write files refuse to overwrite unless the agent passes `"overwrite": true`.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};
use tellshot_capture::{BackendChoice, PhysicalRect};
use tellshot_core::Edit;

use crate::error::{CliError, ErrorKind};
use crate::ops::{self, SchemaKind};

/// Newest first. We answer with the client's version when we support it.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Tellshot captures and annotates screenshots. Typical flow: \
    list_targets → capture (to a .tellshot project) → describe → apply_edits → render_preview \
    to check the result visually → export_png. Coordinates are image pixels from the top-left. \
    Use 'redact' (never blur or pixelate) to hide secrets: export overwrites those pixels. \
    The 'synthetic' backend captures a test pattern anywhere, including headless sandboxes.";

/// Largest accepted request line. Real requests are a few kilobytes; this bounds memory.
const MAX_LINE_BYTES: u64 = 8 << 20;

/// Restricts which files the tools may read and write.
#[derive(Debug, Clone, Default)]
pub(crate) struct Sandbox {
    root: Option<PathBuf>,
}

impl Sandbox {
    /// Confines file access to `root` (and below). `None` allows any path the user can access.
    pub(crate) fn new(root: Option<&Path>) -> std::io::Result<Self> {
        Ok(Self { root: root.map(std::fs::canonicalize).transpose()? })
    }

    /// Resolves `path` and rejects it if it lies outside the root. Paths that do not exist yet
    /// (outputs) are checked through their parent directory.
    fn check(&self, path: &Path) -> Result<PathBuf, CliError> {
        let Some(root) = &self.root else { return Ok(path.to_owned()) };
        let joined = if path.is_absolute() { path.to_owned() } else { root.join(path) };
        let resolved = if let Ok(p) = std::fs::canonicalize(&joined) {
            p
        } else {
            let parent = joined.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(root);
            let name = joined.file_name().ok_or_else(|| outside(path))?;
            std::fs::canonicalize(parent).map_err(|_| outside(path))?.join(name)
        };
        if resolved.starts_with(root) { Ok(resolved) } else { Err(outside(path)) }
    }
}

fn outside(path: &Path) -> CliError {
    CliError::new(
        ErrorKind::InvalidInput,
        format!("{} is outside the allowed folder", path.display()),
    )
    .with_hint("this server was started with --root; use paths inside that folder")
}

/// Serves one session until stdin closes. Malformed lines get a JSON-RPC parse error and the
/// session continues.
///
/// # Errors
/// Only when stdin or stdout fail.
pub(crate) fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    sandbox: &Sandbox,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = std::io::Read::take(&mut input, MAX_LINE_BYTES).read_until(b'\n', &mut buf)?;
        if read == 0 {
            return Ok(());
        }
        let reply = if buf.last() != Some(&b'\n')
            && u64::try_from(read).is_ok_and(|n| n >= MAX_LINE_BYTES)
        {
            // Discard the rest of the oversized line before answering.
            let mut sink = Vec::new();
            input.read_until(b'\n', &mut sink)?;
            Some(rpc_error(&Value::Null, -32600, "request too large"))
        } else {
            match std::str::from_utf8(&buf) {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => handle_line(line, sandbox),
                Err(_) => Some(rpc_error(&Value::Null, -32700, "parse error: invalid UTF-8")),
            }
        };
        if let Some(reply) = reply {
            serde_json::to_writer(&mut output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn handle_line(line: &str, sandbox: &Sandbox) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(rpc_error(&Value::Null, -32700, "parse error"));
    };
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    // Notifications (no id) never get a reply.
    let id = message.get("id")?.clone();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => rpc_ok(&id, &initialize(&params)),
        "ping" => rpc_ok(&id, &json!({})),
        "tools/list" => rpc_ok(&id, &json!({ "tools": tools() })),
        "tools/call" => match call(&params, sandbox) {
            Ok(result) => rpc_ok(&id, &result),
            Err(message) => rpc_error(&id, -32602, &message),
        },
        _ => rpc_error(&id, -32601, &format!("method not found: {method}")),
    })
}

fn rpc_ok(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version =
        PROTOCOL_VERSIONS.iter().find(|v| **v == requested).unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "tellshot", "title": "Tellshot", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

/// Tool definitions live in `tools.json` so they can be reviewed like documentation. The edit
/// schema is generated from the Rust types at runtime, so it can never drift from the code.
fn tools() -> Value {
    let mut tools: Value =
        serde_json::from_str(include_str!("tools.json")).unwrap_or_else(|_| json!([]));
    let edit_schema = ops::schema(SchemaKind::Edit);
    if let Some(apply) =
        tools.as_array_mut().and_then(|list| list.iter_mut().find(|t| t["name"] == "apply_edits"))
        && let Some(input) = apply.get_mut("inputSchema").and_then(Value::as_object_mut)
    {
        input
            .insert("$defs".into(), edit_schema.get("$defs").cloned().unwrap_or_else(|| json!({})));
        if let Some(edits) = input
            .get_mut("properties")
            .and_then(|p| p.get_mut("edits"))
            .and_then(Value::as_object_mut)
        {
            edits.insert(
                "items".into(),
                edit_schema.get("items").cloned().unwrap_or_else(|| json!({})),
            );
        }
    }
    tools
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureArgs {
    output: PathBuf,
    target: Option<String>,
    region: Option<PhysicalRect>,
    #[serde(default)]
    backend: Option<BackendChoice>,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectArgs {
    project: PathBuf,
    #[serde(default)]
    max_size: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditArgs {
    project: PathBuf,
    edits: Vec<Edit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportArgs {
    project: PathBuf,
    output: PathBuf,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct BackendArgs {
    #[serde(default)]
    backend: Option<BackendChoice>,
}

/// Runs a tool. `Err` is a protocol error (unknown tool, malformed arguments); tool failures are
/// reported in the result with `isError: true`, as MCP specifies, so the agent can react.
fn call(params: &Value, sandbox: &Sandbox) -> Result<Value, String> {
    let name = params.get("name").and_then(Value::as_str).ok_or("missing tool name")?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let parse = |e: serde_json::Error| format!("invalid arguments for {name}: {e}");
    let outcome: Result<Value, CliError> = match name {
        "list_targets" => {
            let a: BackendArgs = serde_json::from_value(args).map_err(parse)?;
            ops::targets(a.backend.unwrap_or_default()).map(|t| json!(t))
        }
        "capture" => {
            let a: CaptureArgs = serde_json::from_value(args).map_err(parse)?;
            sandbox
                .check(&a.output)
                .and_then(|output| {
                    ops::capture(ops::CaptureRequest {
                        backend: a.backend.unwrap_or_default(),
                        target: a.target,
                        region: a.region,
                        output,
                        overwrite: a.overwrite,
                    })
                })
                .map(|c| json!(c))
        }
        "describe" => {
            let a: ProjectArgs = serde_json::from_value(args).map_err(parse)?;
            sandbox.check(&a.project).and_then(|p| ops::describe(&p)).map(|d| json!(d))
        }
        "apply_edits" => {
            let a: EditArgs = serde_json::from_value(args).map_err(parse)?;
            sandbox.check(&a.project).and_then(|p| ops::apply_edits(&p, a.edits)).map(|s| json!(s))
        }
        "export_png" => {
            let a: ExportArgs = serde_json::from_value(args).map_err(parse)?;
            sandbox
                .check(&a.project)
                .and_then(|p| Ok((p, sandbox.check(&a.output)?)))
                .and_then(|(p, out)| ops::export(&p, &out, a.overwrite))
                .map(|s| json!(s))
        }
        "render_preview" => {
            let a: ProjectArgs = serde_json::from_value(args).map_err(parse)?;
            let max = a.max_size.unwrap_or(1024).clamp(64, 2048);
            return Ok(match sandbox.check(&a.project).and_then(|p| ops::preview(&p, max)) {
                Ok((png, w, h)) => json!({
                    "content": [
                        { "type": "image", "mimeType": "image/png",
                          "data": base64::engine::general_purpose::STANDARD.encode(png) },
                        { "type": "text", "text": format!("Preview {w}×{h} px") }
                    ]
                }),
                Err(e) => tool_error(&e),
            });
        }
        other => return Err(format!("unknown tool: {other}")),
    };
    Ok(match outcome {
        Ok(data) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&data).unwrap_or_default() }],
            "structuredContent": data,
        }),
        Err(e) => tool_error(&e),
    })
}

fn tool_error(e: &CliError) -> Value {
    json!({
        "content": [{ "type": "text", "text": e.to_string() }],
        "structuredContent": { "error": e },
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(requests: &[Value]) -> Vec<Value> {
        let input = requests.iter().fold(String::new(), |mut acc, r| {
            acc.push_str(&r.to_string());
            acc.push('\n');
            acc
        });
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out, &Sandbox::default()).unwrap();
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn call_tool(id: u32, name: &str, arguments: &Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
    }

    #[test]
    fn handshake_and_tool_listing() {
        let replies = session(&[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" }),
        ]);
        assert_eq!(replies.len(), 3, "notifications get no reply");
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18", "negotiated");
        let names: Vec<&str> = replies[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["list_targets", "capture", "describe", "apply_edits", "render_preview", "export_png"],
            "tools"
        );
        assert_eq!(replies[2]["error"]["code"], -32601, "unknown method");
    }

    #[test]
    fn full_agent_workflow_on_the_synthetic_backend() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("shot.tellshot");
        let png = dir.path().join("shot.png");
        let p = project.to_str().unwrap();
        let replies = session(&[
            call_tool(
                1,
                "capture",
                &json!({ "output": p, "backend": "synthetic", "region": { "x": 0, "y": 0, "width": 400, "height": 300 } }),
            ),
            call_tool(
                2,
                "apply_edits",
                &json!({ "project": p, "edits": [
                { "op": "add", "shape": { "kind": "arrow", "from": { "x": 20, "y": 20 }, "to": { "x": 200, "y": 150 } } },
                { "op": "add", "shape": { "kind": "redact", "rect": { "x": 250, "y": 40, "width": 100, "height": 20 } } }
            ] }),
            ),
            call_tool(3, "describe", &json!({ "project": p })),
            call_tool(4, "render_preview", &json!({ "project": p, "max_size": 200 })),
            call_tool(5, "export_png", &json!({ "project": p, "output": png.to_str().unwrap() })),
            call_tool(6, "export_png", &json!({ "project": p, "output": png.to_str().unwrap() })),
            call_tool(
                7,
                "apply_edits",
                &json!({ "project": p, "edits": [{ "op": "delete", "id": 99 }] }),
            ),
        ]);
        assert_eq!(replies[0]["result"]["structuredContent"]["width"], 400, "captured region");
        assert_eq!(
            replies[1]["result"]["structuredContent"]["applied"][1]["created"], 2,
            "ids returned"
        );
        assert_eq!(
            replies[2]["result"]["structuredContent"]["objects"].as_array().unwrap().len(),
            2,
            "described"
        );
        assert_eq!(replies[3]["result"]["content"][0]["type"], "image", "preview is an image");
        assert!(png.exists(), "exported");
        assert_eq!(replies[5]["result"]["isError"], true, "no silent overwrite");
        assert_eq!(replies[6]["result"]["isError"], true, "bad edit reported to the agent");
        assert_eq!(
            replies[6]["result"]["structuredContent"]["error"]["code"], "invalid_input",
            "stable code"
        );
    }

    #[test]
    fn bad_lines_get_errors_and_the_session_continues() {
        let mut input = b"\xff\xfe not utf8\n{not json}\n".to_vec();
        input.extend_from_slice(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string().as_bytes(),
        );
        input.push(b'\n');
        let mut out = Vec::new();
        serve(input.as_slice(), &mut out, &Sandbox::default()).unwrap();
        let replies: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 3, "one reply per line");
        assert_eq!(replies[0]["error"]["code"], -32700, "invalid UTF-8");
        assert_eq!(replies[1]["error"]["code"], -32700, "invalid JSON");
        assert_eq!(replies[2]["result"], json!({}), "still serving");
    }

    #[test]
    fn root_confines_file_access() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(Some(dir.path())).unwrap();
        assert!(sandbox.check(Path::new("shot.tellshot")).is_ok(), "relative path inside");
        assert!(sandbox.check(&dir.path().join("a.png")).is_ok(), "absolute path inside");
        assert!(sandbox.check(Path::new("/etc/passwd")).is_err(), "outside");
        assert!(sandbox.check(Path::new("../escape.png")).is_err(), "traversal");
    }

    #[test]
    fn malformed_arguments_are_protocol_errors() {
        let replies = session(&[call_tool(1, "capture", &json!({ "outputt": "x.png" }))]);
        assert_eq!(replies[0]["error"]["code"], -32602, "invalid params");
    }
}
