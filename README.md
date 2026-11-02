# Tellshot

[![CI](https://github.com/meetdhruva/tellshot/actions/workflows/ci.yml/badge.svg)](https://github.com/meetdhruva/tellshot/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/meetdhruva/tellshot?include_prereleases)](https://github.com/meetdhruva/tellshot/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#licence)

Fast, reliable screen capture and annotation for Windows, macOS and Linux, written in Rust.
An open-source alternative to Snagit and Snagit Editor.

> **Status: early foundation (v0.1).** The core model, renderer, file format, CLI, MCP server and
> an accessible editor shell work today. Hotkeys, the capture overlay, video and OCR are next; see
> [the roadmap](docs/roadmap.md).

## Why Tellshot

- **Never lose work.** The CLI writes captures before doing anything else; the editor asks before
  closing or replacing unsaved work; every save is atomic; projects are versioned and validated.
- **Non-destructive.** The original pixels are kept untouched; every annotation stays editable and
  every action is undoable.
- **Redaction that actually redacts.** Export overwrites redacted pixels, with no anti-aliased
  edge that could leak a sliver. Blur and pixelate are labelled cosmetic.
- **Accessible from day one.** Keyboard-first, screen-reader names on every control, high-contrast
  and reduced-motion modes, and a colour-blind-safe annotation palette.
- **Built for automation and AI agents.** A scriptable CLI with JSON output and stable exit codes,
  JSON Schema for edits, and an MCP server so agents can capture, annotate, look at and export
  screenshots.

## Install

Download the installer for your platform from the
[latest release](https://github.com/meetdhruva/tellshot/releases/latest):

| Platform | Installer | Portable |
|---|---|---|
| Windows 10+ (x64, ARM64) | `tellshot-<version>-x64-setup.exe` / `arm64-setup.exe` | `.zip` |
| macOS 11+ (Apple silicon, Intel) | `Tellshot-<version>-aarch64.dmg` / `x86_64.dmg` | `.tar.gz` |
| Linux x64, ARM64 (glibc 2.35+) | `tellshot_<version>_amd64.deb` / `arm64.deb` | `.tar.gz` |

Step-by-step instructions, including first launch of the unsigned builds, are in
[docs/install.md](docs/install.md).

To build from source, install Rust (the pinned toolchain installs automatically) and run
`cargo build --release`. On Linux you also need `libfontconfig1-dev` and `libxkbcommon-dev`.

## Try it

```sh
cargo run -p tellshot-cli -- targets --backend synthetic
cargo run -p tellshot-cli -- capture --backend synthetic --region 0,0,640,360 -o shot.tellshot
cargo run -p tellshot-cli -- edit shot.tellshot --edit \
  '{"op":"add","shape":{"kind":"arrow","from":{"x":420,"y":300},"to":{"x":240,"y":140}}}'
cargo run -p tellshot-cli -- export shot.tellshot -o shot.png
cargo run -p tellshot-editor -- shot.tellshot
```

Use `--backend auto` (the default) to capture your real screen on X11, Windows or macOS.

### Use it from an AI agent (MCP)

```json
{ "mcpServers": { "tellshot": { "command": "tellshot", "args": ["mcp", "--root", "/path/to/screenshots"] } } }
```

`--root` limits the server to files in that folder; without it, the agent can read and write any
`.png` or `.tellshot` file you can.

Tools: `list_targets`, `capture`, `describe`, `apply_edits`, `render_preview`, `export_png`.

## Architecture

```
apps/editor (Slint)   apps/cli (CLI + MCP)   apps/agent (planned: tray, hotkeys, overlay)
        │                     │                         │
        └──────────┬──────────┴───────────┬─────────────┘
              tellshot-render       tellshot-capture ─── x11 · xcap (Win/macOS) · synthetic
              tellshot-format       tellshot-ipc
                   │
        tellshot-core (no_std)   tellshot-imaging (no_std)
```

One product with two processes: a small always-running agent (instant hotkeys, overlay, capture)
and an on-demand editor. A crashing editor never loses a capture. Details in
[docs/architecture.md](docs/architecture.md) and the [decision records](docs/adr/).

## Develop

```sh
cargo ci            # every check CI runs: fmt, clippy (strict), tests, docs, deny, typos, machete
cargo xtask bless   # update golden images after an intended rendering change
```

Read [AGENTS.md](AGENTS.md) for the rules (they apply to humans too) and
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The editor uses [Slint](https://slint.dev) under its Royalty-free licence, which requires the
"About Slint" credit in the editor's About window.
