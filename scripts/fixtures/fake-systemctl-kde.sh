#!/usr/bin/env bash
set -euo pipefail

: "${PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD:?isolated wrapper guard missing}"
: "${PORTALDOCTOR_SYSTEMCTL_FAKE_DIR:?fake directory missing}"
: "${PORTALDOCTOR_SYSTEMCTL_MODE_FILE:?mode file missing}"
: "${PORTALDOCTOR_SYSTEMCTL_LOG:?invocation log missing}"
: "${PORTALDOCTOR_SYSTEMCTL_PID_FILE:?PID file missing}"

if [[ "$PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD" != "isolated-kde-systemd" ]]; then
    exit 91
fi

expected=(
    --user
    show
    xdg-desktop-portal-kde.service
    -p ActiveState
    -p SubState
    -p UnitFileState
    --value
)
if (( $# != ${#expected[@]} )); then
    exit 92
fi
for index in "${!expected[@]}"; do
    position=$((index + 1))
    if [[ "${!position}" != "${expected[$index]}" ]]; then
        exit 93
    fi
done

printf '%s\n' "$*" >>"$PORTALDOCTOR_SYSTEMCTL_LOG"
mode="$(sed -n '1p' "$PORTALDOCTOR_SYSTEMCTL_MODE_FILE")"
case "$mode" in
    active)
        printf 'active\nrunning\nstatic\n'
        ;;
    failed)
        printf 'failed\nfailed\nstatic\n'
        ;;
    missing)
        exit 5
        ;;
    timeout)
        printf '%s\n' "$$" >"$PORTALDOCTOR_SYSTEMCTL_PID_FILE"
        sleep 30
        ;;
    *)
        exit 94
        ;;
esac
