# Contributing

Thanks for helping. Three things matter most here: never lose user data, keep everything
accessible, and keep the code easy for the next person (or agent) to change.

## Before you open a pull request

1. `cargo ci` passes locally.
2. New behaviour has a test. Rendering changes have updated golden images (`cargo xtask bless`)
   that you have looked at.
3. Decisions that are expensive to reverse have an ADR in `docs/adr/`.

## Pull request checklist

- [ ] **Reliability**: user data written with `atomic_write`; untrusted input validated; no new
      panicking paths.
- [ ] **UX**: errors say what happened and what to do next; every action can be undone.
- [ ] **Accessibility**: new controls have an accessible role and name, a keyboard path, a visible
      focus ring, and meet 3:1 contrast in light, dark and high-contrast themes.
- [ ] **Agent experience**: new CLI output has a `--json` form; new error cases map to a stable
      error code; MCP tool schemas updated if behaviour changed.
- [ ] **Performance**: hot paths have a benchmark; no regression against the budgets in
      `docs/design/ux-principles.md`.

## Commit style

Imperative subject under 72 characters ("Add pixelate tool"), body explaining why.
