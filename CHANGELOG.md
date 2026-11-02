# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, minor versions may
include breaking changes.

## [Unreleased]

## [0.1.0] - 2026-11-02

First public foundation release. Expect rough edges; see the [roadmap](docs/roadmap.md).

### Added

- `tellshot` command-line tool: `targets`, `capture`, `new`, `describe`, `edit`, `export`,
  `stitch`, `schema` and `mcp`, all with `--json` output and stable error codes.
- MCP server (`tellshot mcp --root <dir>`) so AI agents can capture, annotate, preview and export.
- `tellshot-editor`: annotate captures with arrows, lines, rectangles, ellipses, text, numbered
  steps, highlighter, blur, pixelate, redaction and pen. Keyboard-first, screen-reader names on
  every control, high-contrast and reduced-motion modes.
- Screen capture on X11 (and XWayland), Windows and macOS, plus a synthetic backend for testing.
- `.tellshot` project format: original capture kept untouched, annotations stored as JSON,
  versioned and validated on load, written atomically.
- Redaction that destroys pixels on export, including where blur or pixelate overlap it.
- Scrolling-capture stitching with sticky header and footer detection (`tellshot stitch`).
- Installers: Windows setup (per-user, Start menu, optional PATH and `.tellshot` association),
  macOS `.app` in a `.dmg`, Linux `.deb` with desktop entry, file type, icons and shell
  completions; portable archives for all six targets; checksums and build provenance.
- App icon in every platform format, Windows version information and a high-DPI manifest.
- `tellshot completions <shell>` for bash, zsh, fish, PowerShell and elvish.

### Known limitations

- No global hotkeys, tray icon or region overlay yet; capture from the CLI or the editor.
- Wayland sessions cannot capture yet (XWayland apps excepted).
- Windows and macOS capture is untested on real hardware; binaries are not code-signed or
  notarized.
- On macOS, double-clicking a `.tellshot` file does not open it yet; use Open (Cmd+O) in the editor.
- Text annotations support Latin scripts only.

[Unreleased]: https://github.com/meetdhruva/tellshot/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/meetdhruva/tellshot/releases/tag/v0.1.0
