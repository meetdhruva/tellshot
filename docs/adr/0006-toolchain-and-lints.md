# 0006. Pinned toolchain and strict lints from day one

**Status:** accepted · **Date:** 2026-11-02

## Decision

- `rust-toolchain.toml` pins the latest stable (1.97.0), and `rust-version` matches it. Tellshot
  ships binaries, so an older minimum supported Rust version buys nothing. Bump deliberately.
- `[workspace.lints]` turns on `clippy::pedantic`, `clippy::cargo`, selected `nursery` lints and
  a list of `restriction` lints. Highlights:
  - no `unwrap`/`expect`/`panic!`/`todo!`, panicking indexing or lossy casts in shipped code;
  - `allow_attributes` denied: suppress only with `#[expect(..., reason = "...")]`, so a
    suppression that stops being needed breaks the build;
  - `print_stdout` denied: output goes through locked handles that tolerate closed pipes;
  - `clippy.toml` bans `HashMap`/`HashSet` (non-deterministic order) and raw `fs::write` for user
    data (use `atomic_write`).
- CI runs `-D warnings`, `cargo deny` (licences, advisories, bans), `typos` and `cargo machete`.
- `cargo ci` runs the same steps locally.

## Consequences

Some idioms take more words (`checked_sub`, `get`, `try_from`). Two lint suggestions were wrong
for us and are documented in code: `then_some` evaluates eagerly, which turned a safe subtraction
into an overflow (now `checked_sub`); `tests_outside_test_module` misfires on integration tests
(removed).
