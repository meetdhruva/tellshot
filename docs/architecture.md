# Architecture

## Processes

One product, one installer, two processes:

- **Agent** (planned, `apps/agent`): always running, small. Tray icon, global hotkeys, the
  region-selection overlay (winit + wgpu, frozen-frame), capture. Saves every capture to the
  library before anything else.
- **Editor** (`apps/editor`): opened on demand. If it crashes, no capture is lost.
- They will talk over `tellshot-ipc`: length-prefixed postcard messages with a `Hello`/`Welcome`
  version handshake. The protocol and framing exist and are tested; the transport (per-user Unix
  socket with owner and mode checks, Windows named pipe) arrives with the agent.
- **CLI** (`apps/cli`): the same operations for scripts, plus an MCP server for AI agents.

## Crates and dependency rules

```
apps/*  ──►  render, format, capture, ipc  ──►  core, imaging
```

- `core` and `imaging` are `no_std` + `alloc`: the compiler guarantees they do no I/O. They hold
  every rule worth testing exhaustively (property tests, golden images, benchmarks).
- Platform code exists only in `capture` adapters (and, later, `apps/agent` platform glue).
- Apps never depend on each other.

## Data flow of an edit

```
UI gesture / CLI JSON / MCP call
        │  Edit (intent, untrusted)
        ▼
History::apply ── validate against Limits ── resolve to Op (invertible) ── apply ── record
        │
        ▼
render (export or canvas)        Project::save (atomic)
```

Undo is "apply the inverse op". Consecutive edits from one gesture (a drag, nudges, typing)
coalesce into one undo step.

## Coordinate spaces

| Space | Unit | Where |
|---|---|---|
| Physical desktop | device pixels, may be negative | `capture` |
| Logical | physical ÷ scale factor | UI toolkits |
| Document | one base-image pixel | `core`, `render`, files, edits |

Conversions happen only at boundaries; `core` never sees anything but document space.
