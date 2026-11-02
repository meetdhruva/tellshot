# 0007. Agent experience (AX) is a first-class interface

**Status:** accepted · **Date:** 2026-11-02

## Context

AI agents increasingly operate software for people, and write code in this repository. Both need
predictable, machine-readable interfaces and fast, deterministic feedback.

## Decision

**For agents using Tellshot**

- The CLI and the MCP server call the same `ops.rs` functions, so behaviour never diverges.
- `--json` on every command; errors as `{"error":{"code","message","hint"}}` with stable codes and
  exit statuses.
- Edits are a documented JSON format with a generated JSON Schema (`tellshot schema edit`),
  applied all-or-nothing.
- `render_preview` returns an image so multimodal agents can check their own work.
- Tools that write files refuse to overwrite unless asked, and `--root` confines the server to one
  folder. `apply_edits` returns one result per input edit, so results line up with requests.
- The synthetic backend gives agents a capture source in headless sandboxes.

**For agents developing Tellshot**

- `AGENTS.md` (and `CLAUDE.md`, which imports it) states commands, the crate map and the rules.
- One command, `cargo ci`, runs the same checks as CI. CI additionally runs on three OSes and runs
  the X11 capture test under Xvfb; `cargo ci` skips deny, typos and machete if they are not
  installed and says so.
- Lints explain themselves (`reason = "..."`), tests have assertion messages, and golden-image
  failures write the actual image next to the expected one.
- The `agent-inspect` editor feature embeds Slint's MCP server, so agents can inspect and drive the
  live UI through the same accessibility tree screen readers use.

## Consequences

Error codes, exit statuses, the edit format and MCP tool names are public contracts: additive
changes only, or a version bump.
