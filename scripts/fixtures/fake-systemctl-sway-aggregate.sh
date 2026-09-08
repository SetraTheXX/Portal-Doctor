#!/usr/bin/env bash
set -euo pipefail

: "${PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD:?isolated wrapper guard missing}"
: "${PORTALDOCTOR_SYSTEMCTL_FAKE_DIR:?fake directory missing}"
: "${PORTALDOCTOR_SYSTEMCTL_MODE_FILE:?mode file missing}"
: "${PORTALDOCTOR_SYSTEMCTL_LOG:?invocation log missing}"

if [[ "$PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD" != "isolated-sway-runtime-aggregate" ]]; then
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
    xdg-desktop-portal.service|xdg-desktop-portal-wlr.service|xdg-desktop-portal-gtk.service) ;;
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
    healthy:xdg-desktop-portal.service|wlr-missing:xdg-desktop-portal.service|gtk-missing:xdg-desktop-portal.service|wlr-failed:xdg-desktop-portal.service)
        printf 'active\nrunning\nstatic\n'
        ;;
    healthy:xdg-desktop-portal-wlr.service|gtk-missing:xdg-desktop-portal-wlr.service)
        printf 'active\nrunning\nstatic\n'
        ;;
    wlr-missing:xdg-desktop-portal-wlr.service|wlr-failed:xdg-desktop-portal-wlr.service)
        if [[ "$mode" == "wlr-missing" ]]; then
            exit 5
        fi
        printf 'failed\nfailed\nstatic\n'
        ;;
    healthy:xdg-desktop-portal-gtk.service|wlr-missing:xdg-desktop-portal-gtk.service|wlr-failed:xdg-desktop-portal-gtk.service)
        printf 'active\nrunning\nstatic\n'
        ;;
    gtk-missing:xdg-desktop-portal-gtk.service)
        exit 5
        ;;
    *)
        exit 95
        ;;
esac
