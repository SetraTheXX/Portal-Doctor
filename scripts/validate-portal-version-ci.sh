#!/usr/bin/env bash
set -euo pipefail

command -v cargo >/dev/null
command -v install >/dev/null
command -v timeout >/dev/null

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/portaldoctor-portal-version.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

fake_bin="$test_root/bin"
mkdir -p "$fake_bin"
install -m 0755 "$repo_root/scripts/fixtures/fake-portal-version.sh" "$fake_bin/dpkg-query"

mode_file="$test_root/mode"
log_file="$test_root/invocations"
pid_file="$test_root/timeout.pid"
printf 'exact\n' >"$mode_file"
: >"$log_file"
: >"$pid_file"

export PATH="$fake_bin:${PATH:-}"
export PORTALDOCTOR_VERSION_FAKE_BIN="$fake_bin"
export PORTALDOCTOR_VERSION_FAKE_SCRIPT="$repo_root/scripts/fixtures/fake-portal-version.sh"
export PORTALDOCTOR_VERSION_FAKE_GUARD="isolated-portal-version"
export PORTALDOCTOR_VERSION_FAKE_MODE_FILE="$mode_file"
export PORTALDOCTOR_VERSION_FAKE_LOG="$log_file"
export PORTALDOCTOR_VERSION_FAKE_PID_FILE="$pid_file"

# The ignored test exercises the production frontend-first/package-fallback
# collector. The wrapper is bounded and the fake executable never falls
# through to the host package manager or portal binary.
timeout 45s cargo test --locked --all-features \
    collectors::portal_frontend::tests::isolated_version_collector_matrix \
    -- --exact --ignored --nocapture
