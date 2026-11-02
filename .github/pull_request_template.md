## What and why

<!-- One or two sentences. Link the issue. -->

## Checklist

- [ ] `cargo ci` passes
- [ ] Reliability: user data written with `atomic_write`; untrusted input validated; no new panicking paths
- [ ] UX: errors say what to do next; the action can be undone
- [ ] Accessibility: accessible name and role, keyboard path, visible focus, 3:1 contrast in all themes
- [ ] Agent experience: `--json` output and stable error codes for new CLI behaviour; MCP schemas updated
- [ ] Performance: benchmark added or unchanged for hot paths
- [ ] Docs: ADR for expensive-to-reverse decisions; AGENTS.md updated if a rule changed
