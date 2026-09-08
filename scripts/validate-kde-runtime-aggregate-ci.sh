#!/usr/bin/env bash
set -euo pipefail

command -v cargo >/dev/null
command -v dbus-run-session >/dev/null
command -v timeout >/dev/null
command -v install >/dev/null

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/portaldoctor-kde-runtime.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fake_bin="$test_root/bin"
mkdir -p "$fake_bin"
install -m 0755 "$repo_root/scripts/fixtures/fake-systemctl-kde-aggregate.sh" "$fake_bin/systemctl"

mode_file="$test_root/mode"
log_file="$test_root/invocations"
printf 'healthy\n' >"$mode_file"
: >"$log_file"

export PATH="$fake_bin:${PATH:-}"
export PORTALDOCTOR_SYSTEMCTL_FAKE_DIR="$fake_bin"
export PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD="isolated-kde-runtime-aggregate"
export PORTALDOCTOR_SYSTEMCTL_MODE_FILE="$mode_file"
export PORTALDOCTOR_SYSTEMCTL_LOG="$log_file"

# The Rust test owns only the portal frontend and KDE backend names on this
# private bus; it never starts an XDG portal or an active probe.
timeout 20s dbus-run-session -- cargo test --locked --all-features \
    run::tests::isolated_kde_runtime_collectors_feed_the_passive_rule_pipeline \
    -- --exact --ignored --nocapture
