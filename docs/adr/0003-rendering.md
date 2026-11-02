# 0003. Render exports on the CPU with tiny-skia; add a GPU path later

**Status:** accepted · **Date:** 2026-11-02

## Context

Exports must be deterministic (golden tests, reproducible bug reports, headless servers and CI).
Interactive editing of large images benefits from the GPU.

## Decision

- `tellshot-render` flattens documents with tiny-skia: pure Rust, CPU-only, stable API.
- Order: raster effects (blur, pixelate) on base pixels in z-order → annotations in z-order →
  crop. Redaction writes opaque black over every touched pixel without anti-aliasing.
- Text v0: glyph outlines from one system sans-serif face via `fontdb` + `ttf-parser`, no shaping.
  Latin only. Shaping, fallback and bidirectional text arrive with `parley`.
- The editor currently uses the same renderer for its canvas.

## Consequences

- Golden images compare with a small tolerance for cross-architecture anti-aliasing drift.
- Large captures re-render slowly while dragging; the planned fix is a GPU canvas (Vello or wgpu)
  with tiled caching, keeping this renderer as the export reference.
- Vello CPU was considered but is still pre-1.0; revisit when it stabilises.
