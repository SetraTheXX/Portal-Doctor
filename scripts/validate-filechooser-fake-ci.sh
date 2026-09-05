#!/usr/bin/env bash
set -euo pipefail

binary="${PORTALDOCTOR_BIN:-target/release/portaldoctor}"
harness="${PORTALDOCTOR_FAKE_HARNESS:-scripts/validate-filechooser-fake.py}"
python_bin="${PORTALDOCTOR_PYTHON:-python3}"

test -x "$binary"
test -f "$harness"
command -v dbus-run-session > /dev/null
command -v "$python_bin" > /dev/null

for mode in \
    success \
    close-failure \
    cancel \
    malformed \
    response-timeout \
    late-reply \
    request-timeout \
    transport-failure \
    unsupported; do
    echo "FileChooser controlled lifecycle: $mode"
    dbus-run-session -- "$python_bin" "$harness" \
        --mode "$mode" -- "$binary" probe filechooser --json
done
