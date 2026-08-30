# Security

## Scope

PortalDoctor is a read-only diagnostic CLI. It must never modify the system it
runs on, and it must never require root for normal diagnostics.

## v0.2 design guarantees

- Default commands are passive: no interactive portal dialogs, no state
  mutation and no automatic fixes.
- Only the documented allowlist of 11 diagnostic environment variables is
  collected. `HOME` is read only to derive XDG default roots when the matching
  XDG variables are unset; it is not collected as a diagnostic variable.
- Shareable `portaldoctor report` output applies environment allowlist
  enforcement, `$HOME` normalization, secret-pattern masking and optional
  hostname suppression before terminal, JSON or Markdown serialization. Raw
  journal and PipeWire streams are excluded from the report boundary.
- Review generated reports for local context before attaching them publicly;
  the legacy `portaldoctor check --json` compatibility output is not the
  shareable report boundary and requires manual review.
- Raw arbitrary environment dumps are never emitted.
- External D-Bus and subprocess interactions are bounded by timeouts. Timed-out
  child process groups are killed and reaped so a wedged dependency cannot
  hang the CLI or leave its process tree behind.
- No telemetry, network access or AI dependency exists in the core diagnostic
  path.

## Reporting a vulnerability

Do not open a public issue for security problems. Report vulnerabilities
privately through the GitHub Security Advisories page:

<https://github.com/SetraTheXX/Portal-Doctor/security/advisories/new>

Include `portaldoctor report --format markdown --suppress-hostname` or its JSON
equivalent after reviewing it for local paths and other values that should not
be shared. Never include credentials, API tokens, private keys or unrelated
environment dumps.
