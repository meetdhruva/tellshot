# Accessibility

Target: **WCAG 2.2 AA** for the editor and settings, and accessible output for the people who
receive annotated screenshots.

## Built in now

- **Names and roles** on every control (toolbar, swatches, slider, text field, list, canvas,
  status). The UI test checks that every button, checkbox, slider, list and text field has a name,
  and drives the editor by those names.
- **Keyboard**: every action has a shortcut; every drawing tool works without a pointer; Tab
  reaches every control; Space and Enter activate buttons; visible 2 px focus rings.
- **Annotations list**: every object on the canvas is also a readable list item ("Arrow #3 from
  (420, 300) to (240, 140)"), so screen-reader users can find, select, move and delete annotations.
- **Targets** of at least 36 × 36 px (WCAG 2.2 requires 24 × 24).
- **High contrast** and **Reduce motion** toggles; light and dark follow the system.
- **Annotation colours** use the Okabe–Ito palette, which stays distinguishable with common colour
  vision deficiencies; every swatch has a colour name, never colour alone.
- **Redaction** is real, so screenshots shared for accessibility support do not leak data.

## Known gaps (tracked on the roadmap)

- Status changes are not yet announced as a live region.
- The OS high-contrast and reduced-motion settings are not read yet; the toggles are manual.
- Text annotations are Latin-only until the shaping engine lands (ADR 0003).
- Alt text for exported images (from OCR and annotation descriptions) is planned.

## How to test

- Automated: `cargo test -p tellshot-editor` (UI tests through the accessibility tree).
- Manual, per release: NVDA on Windows, VoiceOver on macOS, Orca on Linux; keyboard-only run of
  capture → annotate → export; 200 % text scaling; high-contrast mode in light and dark.
