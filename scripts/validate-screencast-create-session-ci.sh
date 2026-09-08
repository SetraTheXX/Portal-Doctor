#!/usr/bin/env bash
set -euo pipefail

test -f scripts/validate-screencast-create-session.py
command -v dbus-run-session > /dev/null
command -v python3 > /dev/null

# The internal Rust test starts one isolated fake portal per matrix case. The
# graphical markers only allow the product to reach the D-Bus boundary; no
# compositor or real portal backend is involved.
export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export PORTALDOCTOR_RUN_CONTROLLED_MATRIX=1

dbus-run-session -- cargo test --locked --all-features \
    probes::screencast::tests::controlled_create_session_matrix -- --exact --nocapture
