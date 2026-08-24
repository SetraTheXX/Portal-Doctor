## Summary

<!-- What problem does this PR solve? -->

## Scope

- Roadmap phase / maintenance scope:
- Explicit non-goals:

## Changes

<!-- List the implementation and user-visible behavior. -->

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] `cargo build --release`
- [ ] Real-system validation, if the change affects Linux runtime behavior
- [ ] `scripts/validate-v0.1-faults.py`, if relevant to runtime diagnosis

## Contract and documentation

- [ ] Finding catalog updated, if IDs/severity/evidence changed
- [ ] JSON schema reviewed, if serialized output changed
- [ ] README / release notes / CHANGELOG updated, if user-facing
- [ ] Privacy and read-only impact reviewed

## Safety checklist

- [ ] No root/sudo requirement added
- [ ] No system mutation or interactive portal dialog added to default behavior
- [ ] All external operations are bounded
- [ ] No secrets, credentials or raw environment dumps included
- [ ] No dependency added without justification

## Review notes

<!-- Known limitations, compatibility impact, migration notes or follow-ups. -->
