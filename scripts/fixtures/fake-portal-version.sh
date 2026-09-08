#!/usr/bin/env bash
set -euo pipefail

: "${PORTALDOCTOR_VERSION_FAKE_GUARD:?missing version fake guard}"
test "$PORTALDOCTOR_VERSION_FAKE_GUARD" = "isolated-portal-version"

mode="$(<"$PORTALDOCTOR_VERSION_FAKE_MODE_FILE")"
name="${0##*/}"
printf '%s mode=%s\n' "$name" "$mode" >>"$PORTALDOCTOR_VERSION_FAKE_LOG"

case "$name" in
  dpkg-query)
    if [[ "$#" -ne 3 || "$1" != "-W" || "$2" != '-f=${Version}\n' || "$3" != "xdg-desktop-portal" ]]; then
      printf 'unexpected-argv\n' >>"$PORTALDOCTOR_VERSION_FAKE_LOG"
      exit 90
    fi
    case "$mode" in
      exact) printf '1.22.0\n' ;;
      revision) printf '1.22.0+ds-1ubuntu3\n' ;;
      newer) printf '1.23.0-1ubuntu1\n' ;;
      malformed) printf '1.22.0-git20260909\n' ;;
      nonzero) exit 7 ;;
      timeout)
        printf '%s\n' "$$" >"$PORTALDOCTOR_VERSION_FAKE_PID_FILE"
        sleep 5
        ;;
      oversized) dd if=/dev/zero bs=4096 count=2 2>/dev/null ;;
      unexpected) printf '1.22.0\n1.23.0\n' ;;
      *) exit 91 ;;
    esac
    ;;
  xdg-desktop-portal)
    if [[ "$#" -ne 1 || "$1" != "--version" ]]; then
      printf 'unexpected-argv\n' >>"$PORTALDOCTOR_VERSION_FAKE_LOG"
      exit 90
    fi
    case "$mode" in
      frontend-exact) printf 'xdg-desktop-portal 1.22.0\n' ;;
      frontend-revision) printf 'xdg-desktop-portal 1.22.0+ds-1ubuntu3\n' ;;
      *) exit 91 ;;
    esac
    ;;
  *)
    printf 'unexpected-tool\n' >>"$PORTALDOCTOR_VERSION_FAKE_LOG"
    exit 92
    ;;
esac
