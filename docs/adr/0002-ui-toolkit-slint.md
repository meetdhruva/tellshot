# 0002. Build the editor UI with Slint

**Status:** accepted · **Date:** 2026-11-02

## Context

The editor needs native performance, small memory use, real screen-reader support, a designer
workflow, and a UI that AI agents can inspect. The editing engine (document, renderer) must stay
independent of whichever toolkit we pick.

## Options

| | Accessibility | Notes |
|---|---|---|
| **Slint** | AccessKit-based screen-reader support | Declarative `.slint` compiled ahead of time; live preview; embedded MCP server for agents |
| Tauri 2 + web UI | Browser accessibility tree (strongest) | Two languages; canvas pixels must cross the webview boundary |
| gpui | Unclear | Fastest rendering (Zed), but pre-1.0 with frequent breaking changes |
| iced | Not upstream yet (open issue #552) | Elm architecture; long compile times |

## Decision

Slint, with the editor split into `session.rs` (all behaviour, no Slint types, unit-tested) and
`ui/*.slint` (layout and forwarding only). The capture overlay will be a raw winit + wgpu window
in the agent, independent of Slint.

### Licence

Tellshot is MIT/Apache-2.0. Slint is offered under GPL-3.0, a Royalty-free licence, or a
commercial licence. We use the **Royalty-free 2.0** licence, which allows desktop applications
(open or closed) and requires either the `AboutSlint` widget in an About screen reachable from the
main UI, or the Slint badge on the download page. The editor's About dialog shows `AboutSlint`.
The Royalty-free licence excludes embedded devices and applications that expose Slint's APIs;
Tellshot's plugin API must therefore never re-export Slint types. `deny.toml` permits only this
licence option for Slint crates.

## Consequences

- UI tests find controls by accessible name (`i-slint-backend-testing`), so missing names fail CI.
- The `agent-inspect` feature embeds Slint's MCP server for development (`SLINT_MCP_PORT`).
- If Slint stops fitting, only `apps/editor/ui` and `bind.rs` change; `session.rs` survives.
