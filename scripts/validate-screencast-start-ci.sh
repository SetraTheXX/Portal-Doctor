#!/usr/bin/env bash
set -euo pipefail

test -f scripts/validate-screencast-select-sources.py
command -v dbus-run-session > /dev/null
command -v python3 > /dev/null

# This gate uses only the controlled fake; it never opens a real ScreenCast UI
# and does not exercise StreamsReturned or PipeWire.
export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export PORTALDOCTOR_RUN_CONTROLLED_MATRIX=1

dbus-run-session -- cargo test --locked --all-features \
    probes::screencast::tests::controlled_start_matrix -- --exact --nocapture
