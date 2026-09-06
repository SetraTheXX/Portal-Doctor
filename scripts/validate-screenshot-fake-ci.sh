#!/usr/bin/env bash
set -euo pipefail

binary="${PORTALDOCTOR_BIN:-target/release/portaldoctor}"
harness="${PORTALDOCTOR_SCREENSHOT_FAKE_HARNESS:-scripts/validate-screenshot-fake.py}"
python_bin="${PORTALDOCTOR_PYTHON:-python3}"

test -x "$binary"
test -f "$harness"
command -v dbus-run-session > /dev/null
command -v "$python_bin" > /dev/null

# The fake portal validates protocol lifecycle without a real compositor.
# Mark the child as a graphical session so the product reaches the portal
# boundary while CI remains headless.
export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

for mode in \
    success \
    close-failure \
    cancel \
    malformed \
    malformed-type \
    portal-failure \
    response-timeout \
    late-reply \
    request-timeout \
    transport-failure \
    unsupported-version \
    unsupported-target \
    unavailable; do
    echo "Screenshot controlled lifecycle: $mode"
    dbus-run-session -- "$python_bin" "$harness" \
        --mode "$mode" -- "$binary" probe screenshot --json
done
