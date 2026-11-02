# 0005. The `.tellshot` project format and its versioning

**Status:** accepted · **Date:** 2026-11-02

## Decision

A zip with `manifest.json` (format, `major.minor` version, generator), `document.json` (the
`Document`, pretty-printed) and `base.png` (the untouched capture, stored without recompression).

- **Readable**: JSON is diffable and easy for scripts and agents to inspect.
- **Safe to open**: entry sizes are capped before decompression, image dimensions are bounded, and
  every document invariant is checked on load.
- **Atomic**: written with `atomic_write` (temp file, fsync, rename, directory fsync).

### Versioning rules

- `major`: incompatible change. Readers refuse newer majors with "this file needs a newer
  Tellshot" rather than guessing.
- `minor`: may only add new zip entries, which older readers ignore.
- Because core types use `deny_unknown_fields` (to reject malformed agent input), adding a field
  to `document.json` is a **major** change unless the field is optional with `#[serde(default)]`
  and older readers never see it.

## Consequences

When a major version ships, add a migration from the previous major and a fixture file for it.
