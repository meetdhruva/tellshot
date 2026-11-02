# 0004. One capture trait, one adapter per platform

**Status:** accepted · **Date:** 2026-11-02

## Decision

`tellshot-capture` defines `CaptureBackend` (`targets`, `capture`). Platform code lives only in its
adapter modules; nothing outside the crate sees platform types.

| Backend | Platforms | Status |
|---|---|---|
| `x11` | Linux/BSD on X11 or XWayland | Done: RandR monitors, `GetImage`. Tested under Xvfb in CI. |
| `xcap` | Windows, macOS | Interim, via the `xcap` crate. Compiled in CI; not yet exercised by tests (needs a real display and, on macOS, permission). Converts macOS points to pixels. |
| `synthetic` | Everywhere | Deterministic test pattern for tests, CI and AI agents. |
| Windows.Graphics.Capture + DXGI | Windows | Planned; GPU-resident frames for video. |
| ScreenCaptureKit | macOS | Planned; includes the permission flow. |
| xdg-desktop-portal + PipeWire | Wayland | Planned via `ashpd`; until then Wayland reports a clear error. |

Coordinates are physical desktop pixels (monitors may have negative origins); conversion to
document space happens in the caller.

## Consequences

- Replacing the interim `xcap` backend touches only this crate.
- A shared trait-level conformance suite (targets non-empty, primary first, region clipping,
  `NotFound` for stale ids) is still to be written; every backend should pass it.
