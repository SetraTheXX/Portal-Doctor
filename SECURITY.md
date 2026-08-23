# Security

## Scope

PortalDoctor is a read-only diagnostic CLI. It must never modify the system it
runs on, and must never require root for normal diagnostics.

## Design guarantees

- Default commands are passive: no interactive portal dialogs, no state
  mutation, no automatic fixes.
- Reports are privacy-aware: sensitive values are redacted before rendering;
  raw environment dumps are never emitted in full.
- External interactions (commands, later D-Bus calls) are bounded by timeouts
  so a wedged subsystem cannot hang the tool.
- No telemetry, network access or AI dependency in the core diagnostic path.

## Reporting a vulnerability

Do not open a public issue for security problems. Instead, report the issue
privately to the maintainers via the GitHub security advisories page:

<https://github.com/SetraTheXX/Portal-Doctor/security/advisories/new>

Include the `portaldoctor check --json` output of the affected environment
whenever it is safe to share.