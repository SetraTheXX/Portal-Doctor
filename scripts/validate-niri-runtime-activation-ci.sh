#!/usr/bin/env bash
set -euo pipefail

command -v cargo >/dev/null
command -v dbus-run-session >/dev/null
command -v timeout >/dev/null
command -v install >/dev/null

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/portaldoctor-niri-runtime-activation.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fake_bin="$test_root/bin"
mkdir -p "$fake_bin"
install -m 0755 "$repo_root/scripts/fixtures/fake-systemctl-niri-runtime-activation.sh" "$fake_bin/systemctl"

mode_file="$test_root/mode"
log_file="$test_root/invocations"
pid_file="$test_root/timeout.pid"
printf 'healthy\n' >"$mode_file"
: >"$log_file"
: >"$pid_file"

export PATH="$fake_bin:${PATH:-}"
export PORTALDOCTOR_SYSTEMCTL_FAKE_DIR="$fake_bin"
export PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD="isolated-niri-runtime-activation"
export PORTALDOCTOR_SYSTEMCTL_MODE_FILE="$mode_file"
export PORTALDOCTOR_SYSTEMCTL_LOG="$log_file"
export PORTALDOCTOR_SYSTEMCTL_PID_FILE="$pid_file"
export PORTALDOCTOR_ISOLATED_DBUS=1

# The ignored Rust test uses the production config, D-Bus, systemd-user and
# activation collectors on a private bus. The outer bound prevents a wedged
# collector or subprocess from escaping CI.
timeout 60s dbus-run-session -- cargo test --locked --all-features \
    run::tests::isolated_niri_runtime_and_activation_collectors_feed_passive_rule_pipeline \
    -- --exact --ignored --nocapture
