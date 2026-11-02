# Design tokens

Defined once in `apps/editor/ui/theme.slint` (`Theme` global). Components read tokens; they never
hard-code colours, sizes or durations.

| Group | Tokens | Notes |
|---|---|---|
| Surfaces | `surface`, `surface-raised`, `canvas-backdrop` | Light, dark and high-contrast variants |
| Text | `text`, `text-muted` | Muted text collapses to full contrast in high-contrast mode |
| Lines | `border`, `border-width` | Border width doubles in high-contrast mode |
| Accent | `accent`, `on-accent`, `selection`, `focus-ring` | Focus ring must reach 3:1 on both surfaces |
| Space | `space-1` … `space-4` | 4 px grid |
| Shape | `radius`, `target-size`, `focus-width` | Targets 36 px minimum |
| Type | `font-small`, `font-body`, `font-title` | |
| Motion | `motion-fast`, `motion-medium` | Zero when Reduce motion is on |

Annotation colours are separate: `tellshot_core::palette` (Okabe–Ito), shared by the editor, the
CLI and the renderer so exported images look the same everywhere.
