# PortalDoctor agent entrypoint

Before changing this repository, read
[`docs/PORTALDOCTOR_CURRENT_STATE.md`](docs/PORTALDOCTOR_CURRENT_STATE.md).
It is the canonical handoff for the current release, supported scope and next
bounded task.

## Working rules

- Continue from GitHub Issue #3 and roadmap Phase 8 only. Do not restart the
  historical Phase 0–7 bootstrap sequence.
- Preserve the default command as passive, read-only and bounded. Active portal
  requests must remain explicit, cancellable and opt-in.
- Do not expand to KDE, wlroots/Hyprland/Niri, automatic remediation or GUI work
  before the relevant milestone gate is met.
- Keep the compatibility claim limited to the validated Ubuntu 26.04 + GNOME +
  Wayland + systemd user baseline unless a new validation matrix is added.
- Before declaring a task complete, run the quality gates listed in the current
  state document and update the relevant issue and release documentation.

## Source-of-truth order

1. `docs/PORTALDOCTOR_CURRENT_STATE.md`
2. `docs/PORTALDOCTOR_ASHPD_DECISION.md` for the Phase 8 integration boundary
3. `docs/PORTALDOCTOR_ROADMAP.md`
4. GitHub Issue #3 for the executable v0.3.0 checklist
5. `docs/compatibility.md` and the applicable release notes

Historical sections in the roadmap describe completed bootstrap work and are not
an instruction to repeat it.
