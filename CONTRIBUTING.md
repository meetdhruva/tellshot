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

## Releasing (maintainers)

1. Move the `Unreleased` notes in `CHANGELOG.md` under a new version heading.
2. Bump `version` in the workspace `Cargo.toml` and run `cargo check` to update `Cargo.lock`.
3. Merge to `main`, then tag: `git tag -a v0.2.0 -m "Tellshot 0.2.0" && git push origin v0.2.0`.
4. The `Release` workflow builds every platform, attaches archives, checksums and build
   provenance, and publishes the release with the changelog section as notes. Tags with a
   suffix (`v0.2.0-rc.1`) publish as pre-releases.

## Code of conduct and security

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md). Report vulnerabilities
privately as described in [SECURITY.md](SECURITY.md).

## Commit style

Imperative subject under 72 characters ("Add pixelate tool"), body explaining why.
