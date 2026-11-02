# UX principles

These decide arguments. If a change conflicts with one, change the principle in a pull request
first, with reasons.

1. **The capture is sacred.** Save the raw capture before anything else runs (the CLI does today;
   the agent will). Never block saving on rendering, network or dialogs. Closing, opening another
   file or capturing again with unsaved changes always asks.
2. **Instant means under 100 ms.** Budgets below are measured on mid-range hardware and tracked by
   benchmarks. A feature that misses its budget ships behind a flag until it meets it.
3. **Everything is undoable and nothing is destructive.** The original pixels are never modified;
   a drag, a run of nudges or a burst of typing is one undo step, and undo labels say what will
   happen ("Undo Move arrow").
4. **Keyboard first, pointer welcome.** Every action has a shortcut; every tool works without a
   pointer (Enter inserts the shape at the centre, arrows nudge, Shift+arrows nudge by 10).
5. **Errors say what to do next.** "This is a Wayland session; capture there needs …" beats
   "capture failed". CLI errors carry a `hint`.
6. **Safe by default.** Redaction destroys pixels on export; blur and pixelate are labelled
   cosmetic. Nothing leaves the machine unless the user shares it. No telemetry by default.
7. **Calm interface.** No modal dialogs during capture; motion is short and disabled by "Reduce
   motion"; colour is reserved for state and annotations.

## Latency budgets

| Interaction | Budget | Status |
|---|---|---|
| Hotkey → selection overlay visible | < 100 ms | Agent not built yet |
| Capture → saved to disk | < 200 ms | Fast PNG encode in place |
| Editor cold start → interactive | < 500 ms | Not yet measured |
| Pointer move → canvas updated (1080p) | < 16 ms | Not met with effects: blur ~60 ms on a 2-vCPU VM |
| Export 1080p PNG | < 300 ms | Not yet measured |

## Shortcuts

| Key | Action | Key | Action |
|---|---|---|---|
| V | Select | Ctrl+Z | Undo |
| A | Arrow | Ctrl+Shift+Z, Ctrl+Y | Redo |
| L | Line | Delete, Backspace | Delete selection |
| R | Rectangle | Esc | Deselect |
| O | Ellipse | Enter | Insert current shape at centre |
| T | Text | Arrows (+Shift) | Nudge 1 px (10 px) |
| N | Step number | Ctrl+S | Save |
| H | Highlighter | Ctrl+E | Export PNG |
| B / P / X | Blur / Pixelate / Redact | Ctrl+O | Open |
| D | Pen | | |
