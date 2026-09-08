#!/usr/bin/env bash
set -euo pipefail

test -f scripts/validate-screencast-select-sources.py
command -v dbus-run-session >/dev/null
command -v python3 >/dev/null

# This is the cross-stage controlled gate. It starts only the repository fake
# portal and never opens a real ScreenCast UI or touches PipeWire/media.
export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export PORTALDOCTOR_RUN_CONTROLLED_MATRIX=1

dbus-run-session -- cargo test --locked --all-features \
    probes::screencast::tests::controlled_aggregate_lifecycle_matrix -- --exact --nocapture

probe_help="$(cargo run --locked --quiet -- probe --help)"
if grep -qi "screencast" <<<"$probe_help"; then
    echo "public ScreenCast probe must remain hidden" >&2
    exit 1
fi
