# Roadmap

## Done (v0.1 foundation)

- [x] Workspace, strict lints, `cargo ci`, cargo-deny, typos, machete, CI on three OSes
- [x] Core: document model, 11 annotation kinds, validated edits, undo with gesture coalescing,
      hit testing, property tests
- [x] Imaging: blur, pixelate, secure redaction, scrolling-capture overlap detection and stitching
- [x] Renderer: deterministic CPU export with golden tests; text via system font
- [x] Format: atomic writes, PNG codec with limits, versioned `.tellshot` projects
- [x] Capture: trait, X11 backend (tested under Xvfb), interim Windows/macOS backend, synthetic
- [x] IPC: versioned protocol with a frozen wire format
- [x] CLI with `--json`, JSON Schema, stitching, and an MCP server for AI agents
- [x] Installers for Windows, macOS and Linux (x64 and ARM64), app icon, built on every push
- [x] Editor shell (Slint): all tools, keyboard drawing, annotations list, colours, undo, save,
      export, high contrast, reduced motion, UI tests through the accessibility tree

## Next: capture loop (v0.2)

- [ ] Agent process: tray, global hotkeys, single instance, IPC server, editor supervision
- [ ] Region overlay: frozen frame, magnifier, window snapping (UI Automation / AX / AT-SPI)
- [ ] Native backends: Windows.Graphics.Capture + DXGI; ScreenCaptureKit with permission flow;
      Wayland via xdg-desktop-portal + PipeWire (`ashpd`)
- [ ] Library: SQLite catalogue with FTS5 search and thumbnails
- [ ] Clipboard copy of captures and exports; About dialog polish; settings persistence
- [ ] macOS: open files from Finder (Apple Events), Developer ID signing and notarization;
      Windows Authenticode signing; Flatpak

## Then: smart capture (v0.3)

- [ ] Scrolling capture driven by the agent (stitching algorithm already done)
- [ ] OCR: Windows.Media.Ocr, Apple Vision, `ocrs` fallback; Grab Text; Smart Redact
- [ ] Text shaping and fallback with `parley`, replacing the unmaintained `ttf-parser`
      (RUSTSEC-2026-0192, ignored in `deny.toml` until then); GPU canvas with tiled caching
- [ ] Blur within the 16 ms budget (downscaled blur for large radii, SIMD)
- [ ] Alt text for exports; live-region announcements; OS accessibility preferences

## Later

- [ ] Video: screen + mic + webcam, hardware encoders, fragmented MP4, trim, GIF
- [ ] Step capture, templates, share plugins (WASM components), signed auto-update, packaging
