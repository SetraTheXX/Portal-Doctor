# PortalDoctor — Research & Competitive Analysis

**Status:** Project research baseline  
**Date:** 2026-08-22  
**Project:** PortalDoctor  
**Target:** Linux desktop diagnostics for XDG Desktop Portals, Wayland, PipeWire, WirePlumber and related user-session integration

---

## 1. Executive Summary

PortalDoctor should be built as a **deterministic, read-only-by-default root-cause diagnostic CLI** for Linux desktop portal integration.

The project is not intended to replace `xdg-desktop-portal`, write a new portal backend, replace PipeWire, or provide a generic Linux support assistant. Its job is narrower and more valuable:

> Collect the state of the Linux desktop portal stack, reconstruct how portal backends are selected, correlate runtime evidence, and explain likely configuration/runtime failures with stable finding IDs and explicit evidence.

The ecosystem already has excellent low-level and test tools:

- `busctl` / `dbus-monitor` for D-Bus inspection,
- `systemctl --user` for user-service state,
- `journalctl` for logs,
- `pw-dump` / `pw-top` for PipeWire,
- `wpctl` for WirePlumber,
- ASHPD Demo and KDE portal test applications for exercising portal APIs.

The missing product layer is the **domain-aware correlation and diagnosis layer** that answers “why does this portal path fail on this machine?” instead of merely exposing raw state.

### Recommended project order

PortalDoctor should be the first project in the planned Linux tooling sequence:

1. **PortalDoctor** — desktop/userspace integration diagnostics.
2. **SystemD Doctor** — unit/service root-cause diagnostics.
3. **KernelScope** — process-centric eBPF runtime inspection.

PortalDoctor gives us the reusable engineering pattern required by the later tools:

`collectors -> normalized snapshot -> deterministic rules -> evidence -> report`.

---

## 2. Problem Definition

Modern Linux desktop portal behavior spans several independent components:

```text
Application
   |
   v
org.freedesktop.portal.Desktop
   |
   +--> portal configuration / routing
   |
   +--> desktop-specific portal backend(s)
   |
   +--> D-Bus activation environment
   |
   +--> systemd --user / dbus activation
   |
   +--> Wayland compositor integration
   |
   +--> PipeWire
   |
   +--> WirePlumber
```

A user normally observes only the final symptom:

- screen sharing does not start,
- the chooser opens but no stream appears,
- FileChooser never appears,
- a portal interface is missing,
- a Flatpak/Electron application does not follow theme changes,
- a backend starts in the shell but not through D-Bus activation,
- a portal hangs after display reconfiguration,
- multiple backends provide conflicting behavior.

The actual fault can exist at a completely different layer.

The result is a troubleshooting workflow where users and maintainers manually combine information from unrelated tools and documentation.

PortalDoctor turns this into one reproducible diagnostic operation.

---

## 3. Upstream Architecture Findings

### 3.1 Backend selection is configuration-driven

Current XDG Desktop Portal documentation defines `portals.conf` as the mechanism used to select backend implementations per requested portal interface.

Configuration lookup is affected by:

- `XDG_CURRENT_DESKTOP`,
- `XDG_CONFIG_HOME`,
- `XDG_CONFIG_DIRS`,
- `XDG_DATA_HOME`,
- `XDG_DATA_DIRS`,
- desktop-specific `*-portals.conf`,
- generic `portals.conf`.

A single desktop may deliberately use multiple portal backends. Example conceptually:

```ini
[preferred]
default=gnome;gtk
org.freedesktop.impl.portal.ScreenCast=gnome
org.freedesktop.impl.portal.Secret=gnome-keyring
```

This is important for PortalDoctor because the tool should not use simplistic logic such as:

> “Two portal backends are installed; therefore the system is broken.”

Multiple backends are normal. What matters is which backend is selected for which interface and whether the selected backend can actually provide that interface.

### 3.2 `.portal` metadata is part of the routing model

A backend becomes discoverable by installing a `.portal` descriptor under an XDG Desktop Portal portal metadata directory. The descriptor includes information such as:

- D-Bus name,
- implemented backend interfaces,
- legacy `UseIn` compatibility metadata.

PortalDoctor therefore needs a real backend inventory and cannot infer capabilities from package or process names alone.

### 3.3 D-Bus activation environment is a genuine failure source

XDG Desktop Portal and backend processes are D-Bus activatable session services. Upstream documentation explicitly notes that they inherit variables from the **activation environment**, not automatically from the user's interactive shell.

Variables that may need propagation include:

- `DISPLAY`,
- `WAYLAND_DISPLAY`,
- `XDG_CURRENT_DESKTOP`,
- `XDG_DATA_DIRS`,
- `PATH`,
- `XAUTHORITY`.

This makes a shell-vs-activation environment comparison a high-value PortalDoctor feature.

A system may therefore have:

```text
interactive shell:
  WAYLAND_DISPLAY=wayland-1

systemd user activation environment:
  WAYLAND_DISPLAY=<missing>
```

and the user may incorrectly conclude that “the variable is set, so it cannot be the problem.”

### 3.4 wlroots demonstrates the mixed-backend use case

`xdg-desktop-portal-wlr` intentionally implements only specific interfaces such as Screenshot and ScreenCast and expects other interfaces to be provided by other implementations.

Its documentation also tells users to ensure `WAYLAND_DISPLAY` and `XDG_CURRENT_DESKTOP` reach the D-Bus/systemd activation environment.

This is an ideal future compatibility target because it validates both major PortalDoctor concepts:

- mixed portal backend routing,
- activation-environment diagnostics.

### 3.5 ScreenCast is a multi-stage pipeline

The current ScreenCast portal lifecycle is roughly:

```text
CreateSession
   -> SelectSources
      -> Start
         -> PipeWire stream metadata
            -> OpenPipeWireRemote
```

A diagnostic probe can therefore report exactly which stage fails rather than simply saying “screen share failed.”

The current interface documentation also defines stream information and PipeWire integration semantics. This supports the future active-probe design.

---

## 4. Current Ecosystem Evidence

The problem is not historical or solved.

### 4.1 XDG Desktop Portal remains actively developed

`xdg-desktop-portal` 1.22.1 was released on 2026-06-17 and included security fixes and integration changes. The stack is still evolving, so a diagnostic tool should avoid hardcoding stale assumptions and should expose detected versions in reports.

### 4.2 Real 2026 backend-selection regression

Issue #2033, opened 2026-06-17, reports a Niri environment where duplicate Settings backend resolution under XDG Desktop Portal 1.22.0 caused conflicting `SettingChanged` behavior and broke runtime theme switching.

This is exactly the kind of scenario PortalDoctor should eventually identify as a compatibility-aware warning:

- explicit higher-precedence selection exists,
- multiple implementations are active/resolved,
- the detected XDP version is known to have a relevant behavior/regression,
- a specific interface is affected.

PortalDoctor must distinguish **configuration error** from **known upstream behavior** rather than telling users to blindly delete packages.

### 4.3 Real Ubuntu 26.04 ScreenCast hang

Issue #2091, opened 2026-08-05, reports an Ubuntu 26.04 GNOME/Wayland system where ScreenCast can wedge around `OpenPipeWireRemote` after display reconfiguration.

This reinforces two product requirements:

1. runtime health checks must use timeouts and never hang PortalDoctor indefinitely;
2. future active ScreenCast probing should expose lifecycle stage failure clearly.

---

## 5. Competitive / Adjacent Tool Analysis

There is no need to pretend the ecosystem has no tools. PortalDoctor becomes credible by clearly positioning itself between them.

### 5.1 ASHPD / ASHPD Demo

**What it is**

ASHPD is a Rust wrapper around XDG portal D-Bus interfaces. Its GTK demo previews and exercises many portals and is explicitly intended as a test case/demo for portal behavior and application integration.

**What it does well**

- strongly typed Rust portal access,
- active portal exercises,
- demonstrates real application usage,
- useful reference for future PortalDoctor probes.

**PortalDoctor difference**

```text
ASHPD Demo:
  Can I call/use this portal?

PortalDoctor:
  Why is this portal path unavailable, misrouted, unhealthy, or failing?
```

PortalDoctor should likely use ASHPD later for active probes rather than compete with it at the API-wrapper layer.

### 5.2 KDE portal test application

The KDE XDG Desktop Portal backend repository points to `xdg-portal-test-kde` as a simple test application.

Its purpose is backend/portal testing, not whole-stack root-cause analysis.

PortalDoctor remains complementary.

### 5.3 `busctl`, `gdbus`, `dbus-monitor`

**Strength:** excellent D-Bus visibility.

**Missing layer:** no portal-specific routing model and no correlation with XDG configuration, PipeWire, session variables or likely impact.

PortalDoctor should prefer direct D-Bus queries via Rust/zbus where practical rather than shell-parsing these tools.

### 5.4 `systemctl --user`

**Strength:** authoritative service/unit state on systemd systems.

**Missing layer:** does not answer whether the selected portal backend matches the desktop/interface configuration or whether an environment mismatch explains activation failure.

### 5.5 `journalctl`

**Strength:** structured evidence and historical runtime failures.

**Missing layer:** the user still needs domain knowledge to decide which services matter and which log lines support which diagnosis.

PortalDoctor should collect a bounded, sanitized, relevant subset only.

### 5.6 `pw-dump`

PipeWire documents `pw-dump` as a tool that outputs current PipeWire state in JSON including nodes, devices, modules, ports and other objects.

This is ideal for early PortalDoctor versions because it avoids unnecessary native PipeWire FFI.

### 5.7 `wpctl`

WirePlumber's `wpctl status` exposes devices, nodes, sources, sinks and streams. It is useful for verifying that the PipeWire session-manager side is reachable.

PortalDoctor should initially use this as supporting evidence while keeping the core model focused on portal-relevant health.

---

## 6. Market / GitHub Positioning

PortalDoctor is suitable for a public GitHub project because:

- the target problem is real and current;
- the project can remain Linux-specific without apology;
- the output can help users produce better upstream bug reports;
- the codebase exercises Rust, D-Bus, XDG configuration semantics, Linux desktop integration, structured diagnostics and test fixtures;
- the product has a clear one-sentence purpose;
- the project can grow by desktop environment without rewriting the core.

Recommended positioning:

> **PortalDoctor is a deterministic diagnostic CLI for XDG Desktop Portals, Wayland and PipeWire integration on Linux.**

Alternative longer positioning:

> PortalDoctor inspects portal routing, desktop-session environment, D-Bus activation, user services and PipeWire integration to explain why Linux desktop portals are misconfigured or unhealthy.

### What the project is not

- not a generic “Linux doctor”,
- not an AI support chatbot,
- not a Flatpak manager,
- not a portal backend,
- not a PipeWire replacement,
- not a Wayland compositor helper,
- not a destructive “fix everything” script.

---

## 7. Recommended Initial Target

### v0.1 primary environment

- Ubuntu 26.04
- GNOME
- Wayland
- systemd user session
- XDG Desktop Portal frontend
- GNOME and GTK portal backends

This is intentionally narrow.

The architecture must not hardcode Ubuntu or GNOME into the domain model, but the first validated runtime matrix should use this environment.

### Later compatibility targets

1. KDE Plasma / xdg-desktop-portal-kde
2. Sway / xdg-desktop-portal-wlr
3. Hyprland / xdg-desktop-portal-hyprland
4. Niri / mixed GNOME+GTK portal setups
5. COSMIC where the portal ecosystem warrants explicit handling

---

## 8. Recommended Technology Direction

### Language

**Rust**

Reasons:

- appropriate for Linux tooling,
- single distributable binary,
- strong typed modeling for diagnostic snapshots/findings,
- excellent CLI ecosystem,
- `zbus` provides stable Rust D-Bus access,
- ASHPD is already Rust-native for future active probes.

### Suggested dependencies

Core:

- `clap`
- `serde`
- `serde_json`
- `thiserror`
- `tracing`

Runtime:

- `tokio`
- `zbus`

Later active probes:

- `ashpd`

Avoid early native PipeWire bindings. Parse `pw-dump` JSON first.

---

## 9. Product Principles Derived From Research

### 9.1 Read-only by default

Default execution must only inspect state.

### 9.2 Deterministic core

Same normalized snapshot + same ruleset version should produce the same findings.

### 9.3 Evidence before recommendation

Every non-trivial finding should answer:

- what was observed,
- where it was observed,
- why it matters,
- confidence level,
- recommended next action.

### 9.4 Severity and confidence are different

Example:

- missing ScreenCast provider can be **ERROR / HIGH confidence**;
- suspicious duplicate provider behavior can be **WARNING / MEDIUM confidence**.

### 9.5 No unbounded commands

D-Bus calls, subprocesses and runtime probes need timeouts. A wedged portal must not wedge PortalDoctor.

### 9.6 Privacy by default

Report generation should collect an explicit allowlist, normalize `$HOME`, avoid arbitrary environment dumps and sanitize logs before sharing.

### 9.7 Configuration semantics must be reproduced, not guessed

Portal routing is a first-class engine, not a few regular expressions.

---

## 10. Proposed Finding Families

Stable finding identifiers should exist from early versions.

### Environment

- `ENV001` — `XDG_CURRENT_DESKTOP` missing
- `ENV002` — session type unavailable or inconsistent
- `ENV003` — `WAYLAND_DISPLAY` missing in a Wayland session
- `ENV004` — shell and activation environment mismatch

### XDG Portal

- `XDP001` — portal frontend unavailable
- `XDP002` — portal frontend runtime unhealthy/unreachable
- `XDP003` — no backend descriptors discovered
- `XDP004` — requested portal interface has no provider
- `XDP005` — configured backend is unavailable

### Configuration

- `CFG001` — no matching desktop-specific portal configuration
- `CFG002` — malformed portal configuration
- `CFG003` — explicit preference resolves to an incompatible provider
- `CFG004` — suspicious duplicate/multi-provider resolution

### D-Bus

- `DBUS001` — session bus unavailable
- `DBUS002` — portal or selected backend cannot be activated/reached

### PipeWire

- `PW001` — PipeWire unavailable/unreachable
- `PW002` — WirePlumber unavailable/unreachable
- `PW003` — PipeWire state collection failed

### ScreenCast

- `SC001` — no usable ScreenCast implementation
- `SC002` — ScreenCast route exists but PipeWire path is unavailable

Finding IDs and exact meanings become part of the compatibility contract and should not be casually repurposed.

---

## 11. Research Conclusion

PortalDoctor has a valid open-source niche if it focuses on **explainable diagnosis** rather than portal testing or generic Linux health checks.

The strongest product wedge is:

1. reproduce XDG portal backend routing,
2. compare desktop/session and activation environments,
3. verify D-Bus/user-service runtime,
4. correlate PipeWire/WirePlumber when relevant,
5. produce stable findings and sanitized bug-report-ready output.

This is useful on the developer's own Linux system, appropriate for GitHub, technically meaningful without being kernel-level difficult, and provides an ideal foundation for later Linux diagnostic tooling.

---

## 12. Primary Sources

Research baseline was validated against the following upstream/current sources on 2026-08-22:

1. XDG Desktop Portal — `portals.conf` documentation  
   https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html
2. XDG Desktop Portal — configuration-file overview  
   https://flatpak.github.io/xdg-desktop-portal/docs/configuration-file.html
3. XDG Desktop Portal — system integration / D-Bus activation environment  
   https://flatpak.github.io/xdg-desktop-portal/docs/system-integration.html
4. XDG Desktop Portal — writing a new backend / `.portal` metadata  
   https://flatpak.github.io/xdg-desktop-portal/docs/writing-a-new-backend.html
5. XDG Desktop Portal — ScreenCast API/lifecycle  
   https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
6. XDG Desktop Portal releases  
   https://github.com/flatpak/xdg-desktop-portal/releases
7. XDG Desktop Portal issue #2033 — duplicate Settings backend resolution  
   https://github.com/flatpak/xdg-desktop-portal/issues/2033
8. XDG Desktop Portal issue #2091 — ScreenCast/OpenPipeWireRemote hang on Ubuntu 26.04  
   https://github.com/flatpak/xdg-desktop-portal/issues/2091
9. ASHPD — Rust portal wrapper and demo  
   https://github.com/bilelmoussaoui/ashpd
10. KDE XDG Desktop Portal backend  
    https://github.com/KDE/xdg-desktop-portal-kde
11. wlroots XDG Desktop Portal backend  
    https://github.com/emersion/xdg-desktop-portal-wlr
12. PipeWire `pw-dump`  
    https://docs.pipewire.org/page_man_pw-dump_1.html
13. WirePlumber `wpctl`  
    https://pipewire.pages.freedesktop.org/wireplumber/man/wpctl.html
14. zbus 5.19 documentation  
    https://docs.rs/zbus/latest/zbus/

