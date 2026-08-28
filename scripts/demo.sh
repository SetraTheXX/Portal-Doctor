#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${PORTALDOCTOR_BIN:-$ROOT_DIR/target/release/portaldoctor}"

if [[ ! -x "$BINARY" ]]; then
    printf 'Missing executable: %s\nBuild with: cargo build --release\n' "$BINARY" >&2
    exit 1
fi

# Avoid relying on TERM being set when the demo is smoke-tested outside a PTY.
printf '\033[2J\033[H'
printf '\033[1;36mPortalDoctor\033[0m — Linux portal diagnostics in seconds\n\n'
printf '\033[2m$ portaldoctor\033[0m\n'
"$BINARY"
sleep 1

printf '\n\033[2m$ portaldoctor portal explain ScreenCast\033[0m\n'
"$BINARY" portal explain ScreenCast
sleep 1

printf '\n\033[2m$ env -u WAYLAND_DISPLAY portaldoctor check environment\033[0m\n'
printf '\033[2m(simulated missing Wayland display)\033[0m\n'
env -u WAYLAND_DISPLAY "$BINARY" check environment
sleep 2
