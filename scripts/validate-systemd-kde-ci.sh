#!/usr/bin/env bash
set -euo pipefail

command -v cargo >/dev/null
command -v timeout >/dev/null
command -v install >/dev/null

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/portaldoctor-systemd-kde.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fake_bin="$test_root/bin"
mkdir -p "$fake_bin"
install -m 0755 "$repo_root/scripts/fixtures/fake-systemctl-kde.sh" "$fake_bin/systemctl"

mode_file="$test_root/mode"
log_file="$test_root/invocations"
pid_file="$test_root/timeout.pid"
printf 'active\n' >"$mode_file"
: >"$log_file"
: >"$pid_file"

export PATH="$fake_bin:${PATH:-}"
export PORTALDOCTOR_SYSTEMCTL_FAKE_DIR="$fake_bin"
export PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD="isolated-kde-systemd"
export PORTALDOCTOR_SYSTEMCTL_MODE_FILE="$mode_file"
export PORTALDOCTOR_SYSTEMCTL_LOG="$log_file"
export PORTALDOCTOR_SYSTEMCTL_PID_FILE="$pid_file"

# This ignored test is the only code allowed to use the fake systemctl. The
# outer bound protects CI if the Rust test itself or cargo becomes wedged.
timeout 20s cargo test --locked --all-features \
    collectors::systemd_user::tests::isolated_kde_systemd_collector_contract \
    -- --exact --ignored --nocapture
