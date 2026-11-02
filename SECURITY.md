# Security policy

Tellshot handles screenshots, which often contain private information, so we take security
reports seriously.

## Supported versions

Only the latest release receives security fixes while the project is pre-1.0.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through
[GitHub private vulnerability reporting](https://github.com/meetdhruva/tellshot/security/advisories/new).

Include what you found, how to reproduce it, and the impact you expect. We aim to acknowledge
reports within 3 working days and to agree a disclosure date with you once a fix is ready.

## In scope

- Redaction that leaves recoverable pixels or metadata in exported files.
- Crashes, hangs or excessive memory use from opening a crafted `.tellshot`, PNG or IPC message.
- The MCP server reading or writing files outside the folder given to `--root`.
- Anything that sends captures off the machine without the user asking.
