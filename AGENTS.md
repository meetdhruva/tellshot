# AGENTS.md

Instructions for AI coding agents (and humans in a hurry) working on Tellshot.
Read this before changing code. Keep it short and true; update it when a rule changes.

## Verify before you finish

```sh
cargo ci                 # = cargo xtask ci: fmt, clippy -D warnings, tests, docs, deny, typos, machete
cargo test -p <crate>    # one crate while iterating
cargo xtask bless        # regenerate golden images after an INTENDED rendering change; review the diff
xvfb-run -a cargo test -p tellshot-capture --test x11   # real X11 capture test (Linux)
```

A change is done only when `cargo ci` passes. Do not weaken a lint, a test or a limit to make it pass.

## Map

| Path | What | Rules |
|---|---|---|
| `crates/core` | Document model, `Edit` → `Op`, undo `History`, geometry | `no_std`: no I/O, no clock, no platform code. All input validated against `Limits`. |
| `crates/imaging` | Pixel algorithms: blur, pixelate, secure fill, scroll stitching | `no_std`. Benchmarks in `benches/`. |
| `crates/render` | Deterministic CPU export renderer (tiny-skia) | Golden tests in `tests/golden.rs`, fixtures in `fixtures/golden/`. |
| `crates/format` | `atomic_write`, PNG codec, `.tellshot` zip format | The only way user data is written. Versioned; see ADR 0005. |
| `crates/capture` | `CaptureBackend` trait; x11, xcap (Win/macOS), synthetic | Only crate with `cfg(target_os)` besides `apps/editor` platform glue. |
| `crates/ipc` | Agent ↔ client protocol | Append enum variants only; `wire_format_is_frozen` test enforces it. |
| `apps/cli` | `tellshot` binary: CLI + MCP server | One implementation in `ops.rs` serves both. Stable error codes in `error.rs`. |
| `apps/editor` | Slint editor | `session.rs` holds all behaviour (unit-tested, no Slint); `ui/*.slint` is layout only. |
| `xtask` | Repo automation | Rust, not shell, so it runs on every OS. |
| `docs/adr` | Why things are the way they are | Add an ADR for any decision that is expensive to reverse. |

Dependencies point inward: apps → service crates → `core`/`imaging`. Never the reverse.

## Code rules (enforced by lints; see `[workspace.lints]` and `clippy.toml`)

- No `unwrap`, `expect`, `panic!`, `todo!`, indexing that can panic, or `as` casts that truncate in
  shipped code. Tests may panic.
- Suppress a lint only with `#[expect(lint, reason = "…")]`. `#[allow]` is denied, so stale
  suppressions fail the build.
- No `HashMap`/`HashSet` (random order breaks deterministic files and tests); use `BTree*`.
- Never `std::fs::write` or `File::create` for user data; use `tellshot_format::atomic_write`.
- Every document change goes through `History::apply(Edit)`. Never mutate a `Document` directly.
- New `Edit`/`Shape` fields need `#[serde(default)]` or a format major-version bump (ADR 0005).
- `unsafe` is denied workspace-wide; platform crates may opt in per block with a `// SAFETY:` comment.
- Modules are `foo.rs` + `foo/`, never `foo/mod.rs`.

## UI rules (apps/editor)

- Every interactive element has `accessible-role` and `accessible-label`. The UI test
  `editor_is_fully_operable_through_accessibility_and_keyboard` (`apps/editor/tests/ui.rs`) fails
  if any button, checkbox, slider, list or text field lacks a name.
- Every action has a keyboard path. Every tool works without a pointer (Enter inserts, arrows nudge).
- Colours, spacing, type and motion come from `ui/theme.slint` tokens only. Check contrast in both
  themes and in high-contrast mode.
- Behaviour belongs in `session.rs` with a unit test; `.slint` files only lay out and forward.

## Driving the app as an agent

- CLI: every command takes `--json`; errors are `{"error":{"code","message","hint"}}` with stable
  exit codes (3 invalid input, 4 not found, 5 already exists, 6 capture unavailable, 7 I/O).
- `tellshot schema edit` prints the JSON Schema for edits.
- `tellshot mcp --root <dir>` is an MCP server (stdio) with `capture`, `describe`, `apply_edits`,
  `render_preview` (returns a PNG you can look at) and `export_png`; `--root` confines file access.
- `--backend synthetic` gives a deterministic fake desktop; use it in sandboxes and tests.
- Editor UI: build with `--features agent-inspect` and run with `SLINT_MCP_PORT=8765` to inspect and
  click through the live UI via its accessibility tree (localhost only; never in releases).
