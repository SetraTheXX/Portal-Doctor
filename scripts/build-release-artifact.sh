#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  printf 'usage: %s EMPTY_ARTIFACT_DIR\n' "${BASH_SOURCE[0]}" >&2
  exit 2
fi

command -v cargo >/dev/null
command -v find >/dev/null
command -v install >/dev/null
command -v python3 >/dev/null
command -v rustc >/dev/null
command -v sha256sum >/dev/null

repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
artifact_dir="$1"

if [[ -z "$artifact_dir" || "$artifact_dir" == "/" ]]; then
  printf 'artifact directory must be a non-root path\n' >&2
  exit 2
fi

if [[ -e "$artifact_dir" && ! -d "$artifact_dir" ]]; then
  printf 'artifact path is not a directory: %s\n' "$artifact_dir" >&2
  exit 2
fi
mkdir -p -- "$artifact_dir"

# Requiring an empty, caller-owned directory prevents stale or unrelated files
# from being silently included in a release payload.
if [[ -n "$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'artifact directory must be empty: %s\n' "$artifact_dir" >&2
  exit 2
fi

cd "$repo_root"

version="$(
  cargo metadata --locked --no-deps --format-version 1 |
    python3 -c '
import json
import sys

packages = [
    package
    for package in json.load(sys.stdin)["packages"]
    if package["name"] == "portaldoctor"
]
if len(packages) != 1:
    raise SystemExit("expected exactly one portaldoctor package")
print(packages[0]["version"])
'
)"

if [[ ! "$version" =~ ^[0-9]+(\.[0-9]+){2}([+-][0-9A-Za-z.-]+)?$ ]]; then
  printf 'unsupported Cargo package version for artifact naming: %s\n' "$version" >&2
  exit 2
fi

target="$(rustc -vV | awk '$1 == "host:" { print $2 }')"
case "$target" in
  x86_64-unknown-linux-*) ;;
  *)
    printf 'release artifact gate requires an x86_64 Linux host, got: %s\n' "$target" >&2
    exit 2
    ;;
esac

CARGO_TARGET_DIR="$repo_root/target" cargo build --locked --release --target "$target"

source_binary="$repo_root/target/$target/release/portaldoctor"
if [[ ! -f "$source_binary" || ! -x "$source_binary" ]]; then
  printf 'release binary is missing or not executable: %s\n' "$source_binary" >&2
  exit 1
fi

artifact_name="portaldoctor-${version}-${target}"
checksum_name="${artifact_name}.sha256"
artifact_path="$artifact_dir/$artifact_name"

install -m 0755 -- "$source_binary" "$artifact_path"
if [[ ! -x "$artifact_path" ]]; then
  printf 'staged artifact is not executable: %s\n' "$artifact_name" >&2
  exit 1
fi

version_output="$("$artifact_path" --version)"
if [[ "$version_output" != "portaldoctor $version" ]]; then
  printf 'artifact version mismatch: expected portaldoctor %s, got %s\n' \
    "$version" "$version_output" >&2
  exit 1
fi

(
  cd "$artifact_dir"
  sha256sum -- "$artifact_name" >"$checksum_name"
  sha256sum -c -- "$checksum_name" >/dev/null
)

install_root="$(mktemp -d "${TMPDIR:-/tmp}/portaldoctor-artifact-install.XXXXXX")"
trap 'rm -rf "$install_root"' EXIT
mkdir -p -- "$install_root/bin"
install -m 0755 -- "$artifact_path" "$install_root/bin/portaldoctor"
test -x "$install_root/bin/portaldoctor"
test "$("$install_root/bin/portaldoctor" --version)" = "portaldoctor $version"

entries="$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort)"
expected_entries="$(printf '%s\n%s\n' "$artifact_name" "$checksum_name" | sort)"
if [[ "$entries" != "$expected_entries" ]]; then
  printf 'artifact directory contains unexpected entries\n' >&2
  exit 1
fi

printf 'artifact=%s\nchecksum=%s\ntarget=%s\nversion=%s\n' \
  "$artifact_name" "$checksum_name" "$target" "$version"
