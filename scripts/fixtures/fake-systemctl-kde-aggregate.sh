#!/usr/bin/env bash
set -euo pipefail

: "${PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD:?isolated wrapper guard missing}"
: "${PORTALDOCTOR_SYSTEMCTL_FAKE_DIR:?fake directory missing}"
: "${PORTALDOCTOR_SYSTEMCTL_MODE_FILE:?mode file missing}"
: "${PORTALDOCTOR_SYSTEMCTL_LOG:?invocation log missing}"

if [[ "$PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD" != "isolated-kde-runtime-aggregate" ]]; then
    exit 91
fi

expected=(
    --user
    show
    ''
    -p ActiveState
    -p SubState
    -p UnitFileState
    --value
)
if (( $# != ${#expected[@]} )); then
    exit 92
fi
case "$3" in
    xdg-desktop-portal.service|xdg-desktop-portal-kde.service) ;;
    *) exit 93 ;;
esac
expected[2]="$3"
for index in "${!expected[@]}"; do
    position=$((index + 1))
    if [[ "${!position}" != "${expected[$index]}" ]]; then
        exit 94
    fi
done

printf '%s\n' "$*" >>"$PORTALDOCTOR_SYSTEMCTL_LOG"
mode="$(sed -n '1p' "$PORTALDOCTOR_SYSTEMCTL_MODE_FILE")"
case "$mode:$3" in
    healthy:xdg-desktop-portal.service|missing:xdg-desktop-portal.service|failed:xdg-desktop-portal.service)
        printf 'active\nrunning\nstatic\n'
        ;;
    healthy:xdg-desktop-portal-kde.service)
        printf 'active\nrunning\nstatic\n'
        ;;
    missing:xdg-desktop-portal-kde.service)
        exit 5
        ;;
    failed:xdg-desktop-portal-kde.service)
        printf 'failed\nfailed\nstatic\n'
        ;;
    *)
        exit 95
        ;;
esac
