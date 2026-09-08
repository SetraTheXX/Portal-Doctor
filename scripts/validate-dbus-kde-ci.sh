#!/usr/bin/env bash
set -euo pipefail

command -v dbus-run-session >/dev/null
command -v timeout >/dev/null

# The ignored Rust test acquires only the KDE well-known name on this private
# bus and calls the production D-Bus collector. It never starts a portal,
# touches systemd, or opens an active probe.
export PORTALDOCTOR_ISOLATED_DBUS=1
timeout 20s dbus-run-session -- cargo test --locked --all-features \
    collectors::dbus::tests::isolated_kde_backend_ownership_and_determinism \
    -- --exact --ignored --nocapture
