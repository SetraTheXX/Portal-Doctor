#!/usr/bin/env bash
set -euo pipefail

: "${PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD:?isolated wrapper guard missing}"
: "${PORTALDOCTOR_SYSTEMCTL_MODE_FILE:?mode file missing}"
: "${PORTALDOCTOR_SYSTEMCTL_LOG:?invocation log missing}"
: "${PORTALDOCTOR_SYSTEMCTL_PID_FILE:?timeout PID file missing}"

if [[ "$PORTALDOCTOR_SYSTEMCTL_FAKE_GUARD" != "isolated-sway-activation" ]]; then
    exit 91
fi

mode="$(sed -n '1p' "$PORTALDOCTOR_SYSTEMCTL_MODE_FILE")"
if (( $# == 2 )) && [[ "$1" == "--user" && "$2" == "show-environment" ]]; then
    printf '%s\n' "$*" >>"$PORTALDOCTOR_SYSTEMCTL_LOG"
    case "$mode" in
        healthy|stale-desktop|stale-wayland|missing-wayland)
            if [[ "$mode" == "stale-desktop" ]]; then
                printf 'XDG_CURRENT_DESKTOP=GNOME\n'
            else
                printf 'XDG_CURRENT_DESKTOP=Sway\n'
            fi
            printf 'XDG_SESSION_DESKTOP=sway\n'
            printf 'XDG_SESSION_TYPE=wayland\n'
            case "$mode" in
                stale-wayland) printf 'WAYLAND_DISPLAY=wayland-0\n' ;;
                missing-wayland) ;;
                *) printf 'WAYLAND_DISPLAY=wayland-1\n' ;;
            esac
            ;;
        unavailable)
            exit 5
            ;;
        timeout)
            printf '%s\n' "$$" >"$PORTALDOCTOR_SYSTEMCTL_PID_FILE"
            sleep 5
            ;;
        *)
            exit 95
            ;;
    esac
    exit 0
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
printf 'active\nrunning\nstatic\n'
