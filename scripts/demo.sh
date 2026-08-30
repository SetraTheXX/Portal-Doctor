#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${PORTALDOCTOR_BIN:-$ROOT_DIR/target/release/portaldoctor}"

if [[ ! -x "$BINARY" ]]; then
    printf 'Missing executable: %s\nBuild with: cargo build --release\n' "$BINARY" >&2
    exit 1
fi

# The demo is intentionally styled here instead of changing the CLI's plain
# output contract. This keeps README captures expressive while JSON and normal
# terminal output remain stable for scripts and CI.
RESET=$'\033[0m'
BOLD=$'\033[1m'
DIM=$'\033[2m'
CYAN=$'\033[38;5;116m'
BLUE=$'\033[38;5;111m'
GREEN=$'\033[38;5;114m'
YELLOW=$'\033[38;5;221m'
MAGENTA=$'\033[38;5;183m'
WHITE=$'\033[38;5;255m'

clear_screen() {
    # Do not rely on TERM being set when this script is smoke-tested outside a
    # real terminal. Terminalizer records these ANSI sequences correctly.
    printf '\033[2J\033[H'
}

header() {
    printf '%s\n' "${BOLD}${CYAN}PortalDoctor${RESET} ${DIM}· explainable Linux desktop diagnostics${RESET}"
    printf '%s\n\n' "${DIM}Wayland · XDG portals · D-Bus · systemd user session${RESET}"
}

command_line() {
    printf '%s\n' "${MAGENTA}${BOLD}❯${RESET} ${DIM}$1${RESET}"
}

colorize_report() {
    while IFS= read -r line; do
        case "$line" in
            "PortalDoctor "*)
                printf '%s\n' "${BOLD}${CYAN}${line}${RESET}" ;;
            "Snapshot schema "*)
                printf '%s\n' "${DIM}${line}${RESET}" ;;
            "System: "*|"Session: "*)
                printf '%s\n' "${BLUE}${line}${RESET}" ;;
            "Activation environment: consistent"*)
                printf '%s\n' "${GREEN}${line}${RESET}" ;;
            "Activation environment: "*)
                printf '%s\n' "${YELLOW}${line}${RESET}" ;;
            "D-Bus: connected · portal frontend reachable"*)
                printf '%s\n' "${GREEN}${line}${RESET}" ;;
            "D-Bus: "*)
                printf '%s\n' "${YELLOW}${line}${RESET}" ;;
            "PipeWire: reachable"*|"WirePlumber: reachable"*)
                printf '%s\n' "${GREEN}${line}${RESET}" ;;
            "PipeWire: "*|"WirePlumber: "*)
                printf '%s\n' "${YELLOW}${line}${RESET}" ;;
            "  video sources: "*)
                printf '%s\n' "${CYAN}${line}${RESET}" ;;
            "  backend "*": reachable"*)
                printf '%s\n' "${GREEN}${line}${RESET}" ;;
            "  backend "*)
                printf '%s\n' "${YELLOW}${line}${RESET}" ;;
            "Findings: none detected."*)
                printf '%s\n' "${BOLD}${GREEN}${line}${RESET}" ;;
            "Findings: "*)
                printf '%s\n' "${BOLD}${YELLOW}${line}${RESET}" ;;
            "  [WARNING]"*)
                printf '%s\n' "${BOLD}${YELLOW}${line}${RESET}" ;;
            "    next: "*)
                printf '%s\n' "${BLUE}${line}${RESET}" ;;
            "Interface: "*|"Status: selected"*|"Requested: "*|"Available: "*|"Selected: "*)
                printf '%s\n' "${CYAN}${line}${RESET}" ;;
            "Evidence:"*)
                printf '%s\n' "${MAGENTA}${BOLD}${line}${RESET}" ;;
            "  - "*)
                printf '%s\n' "${DIM}${line}${RESET}" ;;
            *)
                printf '%s\n' "$line" ;;
        esac
    done
}

run_report() {
    "$BINARY" "$@" | colorize_report
}

run_environment_fault() {
    env -u WAYLAND_DISPLAY "$BINARY" check environment | colorize_report
}

clear_screen
header
printf '%s\n' "${BLUE}${BOLD}[1/3]${RESET} ${WHITE}${BOLD}HEALTH CHECK${RESET}"
command_line '$ portaldoctor'
sleep 1.2
run_report
sleep 3.2

clear_screen
header
printf '%s\n' "${BLUE}${BOLD}[2/3]${RESET} ${WHITE}${BOLD}ROUTING EXPLAINED${RESET}"
command_line '$ portaldoctor portal explain ScreenCast'
sleep 1.2
run_report portal explain ScreenCast
sleep 3.2

clear_screen
header
printf '%s\n' "${BLUE}${BOLD}[3/3]${RESET} ${WHITE}${BOLD}CONTROLLED FAULT${RESET}"
command_line '$ env -u WAYLAND_DISPLAY portaldoctor check environment'
printf '%s\n\n' "${DIM}simulated missing Wayland display${RESET}"
sleep 1.2
run_environment_fault
sleep 4.2
# Emit a final invisible reset after the pause so the last diagnostic frame is
# held long enough to read before the recording exits.
printf '%s' "$RESET"
