#!/usr/bin/env bash
set -euo pipefail
test -f scripts/validate-screencast-select-sources.py
command -v dbus-run-session >/dev/null
export WAYLAND_DISPLAY=controlled-only
export PORTALDOCTOR_RUN_CONTROLLED_MATRIX=1
dbus-run-session -- cargo test --locked --all-features \
    probes::screencast::tests::controlled_streams_returned_matrix -- --exact --nocapture
