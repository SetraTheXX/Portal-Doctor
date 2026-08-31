#!/usr/bin/env python3
"""End-to-end v0.1-compatible fault-injection acceptance validation.

The harness changes only child-process environments and temporary files. It
never edits system XDG configuration, stops services, or writes repository
state.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable


REQUIRED_FINDING_FIELDS = {
    "id",
    "severity",
    "confidence",
    "title",
    "summary",
    "explanation",
    "evidence",
    "impact",
    "recommendation",
    "source_component",
}

EXPECTED_FINDING_CONTRACT = {
    "ENV001": ("warning", "high", "environment_mismatch"),
    "ENV003": ("warning", "high", "environment_mismatch"),
    "CFG002": ("warning", "high", "config_selection"),
    "XDP003": ("warning", "high", "missing_provider"),
    "XDP005": ("warning", "high", "config_selection"),
    "DBUS001": ("warning", "high", "missing_provider"),
    "XDP001": ("warning", "high", "missing_provider"),
    "DBUS002": ("warning", "high", "service_state"),
}


def binary_path() -> str:
    configured = os.environ.get("PORTALDOCTOR_BIN")
    if configured:
        return configured
    root = Path(__file__).resolve().parents[1]
    # Prefer the binary produced by this checkout. A globally installed
    # portaldoctor may be stale and would make local acceptance tests validate
    # a different source tree than the one being changed.
    for candidate in (
        root / "target" / "release" / "portaldoctor",
        root / "target" / "debug" / "portaldoctor",
    ):
        if candidate.is_file():
            return str(candidate)
    installed = shutil.which("portaldoctor")
    if installed:
        return installed
    raise RuntimeError("portaldoctor bulunamadı; PORTALDOCTOR_BIN ayarlayın")


def desktops(env: dict[str, str]) -> list[str]:
    raw = env.get("XDG_CURRENT_DESKTOP", "")
    return [part.strip().lower() for part in raw.split(":") if part.strip()]


def fixture_environment(env: dict[str, str]) -> dict[str, str]:
    """Fill session defaults so fault scenarios are portable to CI runners."""
    env.setdefault("XDG_CURRENT_DESKTOP", "GNOME")
    env.setdefault("XDG_SESSION_DESKTOP", "GNOME")
    env.setdefault("XDG_SESSION_TYPE", "wayland")
    env.setdefault("WAYLAND_DISPLAY", "wayland-0")
    env.setdefault("DISPLAY", ":0")
    return env


def run_json(
    binary: str,
    env: dict[str, str],
    *args: str,
    expected_exit: int = 0,
) -> tuple[dict, str]:
    result = subprocess.run(
        [binary, *args, "--json"],
        env=env,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    if result.returncode != expected_exit:
        raise AssertionError(
            f"beklenen exit {expected_exit}, alınan {result.returncode}: "
            f"{result.stderr.strip()}"
        )
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise AssertionError(f"JSON parse edilemedi: {exc}: {result.stdout}") from exc
    validate_finding_contract(value)
    return value, result.stderr


def validate_finding_contract(value: dict) -> None:
    assert set(value) == {
        "schema_version",
        "portaldoctor_version",
        "snapshot",
        "findings",
    }
    assert value["schema_version"] == 1
    assert isinstance(value["findings"], list)
    for finding in value["findings"]:
        assert set(finding) == REQUIRED_FINDING_FIELDS
        assert finding["evidence"]
        assert finding["recommendation"]
        assert finding["recommendation"][0]


def finding_ids(value: dict) -> list[str]:
    return [finding["id"] for finding in value["findings"]]


def assert_expected_contract(value: dict, finding_id: str) -> dict:
    finding = next(
        (item for item in value["findings"] if item["id"] == finding_id),
        None,
    )
    assert finding is not None, f"{finding_id} finding yok"
    severity, confidence, evidence = EXPECTED_FINDING_CONTRACT[finding_id]
    assert finding["severity"] == severity, (finding_id, finding["severity"])
    assert finding["confidence"] == confidence, (finding_id, finding["confidence"])
    assert evidence in finding["evidence"], (finding_id, finding["evidence"])
    return finding


def normalized(value: dict) -> dict:
    copy = json.loads(json.dumps(value))
    copy["snapshot"]["collected_at"] = 0
    process = copy["snapshot"].get("environment", {}).get("value")
    if process and "process" in process:
        process["process"].pop("DBUS_SESSION_BUS_ADDRESS", None)
    return copy


def run_repeated(
    name: str,
    binary: str,
    env: dict[str, str],
    expected: str | None,
    configure: Callable[[Path, dict[str, str]], None] | None = None,
    args: tuple[str, ...] = ("check",),
    extra_check: Callable[[dict, Path], None] | None = None,
    expected_exit: int = 0,
) -> None:
    with tempfile.TemporaryDirectory(prefix=f"portaldoctor-{name}-") as temp:
        root = Path(temp)
        scenario_env = env.copy()
        if configure:
            configure(root, scenario_env)
        values: list[dict] = []
        stderr_values: list[str] = []
        for _ in range(3):
            value, stderr = run_json(
                binary, scenario_env, *args, expected_exit=expected_exit
            )
            values.append(value)
            stderr_values.append(stderr)
        ids = finding_ids(values[0])
        expected_finding = None
        if expected:
            assert expected in ids, f"{name}: {expected} yok; actual={ids}"
            expected_finding = assert_expected_contract(values[0], expected)
            text_result = subprocess.run(
                [binary, *args],
                env=scenario_env,
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            assert text_result.returncode == expected_exit, text_result.stderr
            next_line = f"next: {expected_finding['recommendation'][0]}"
            assert next_line in text_result.stdout, (
                f"{name}: terse output did not expose the expected next recommendation"
            )
        for value in values[1:]:
            assert normalized(value) == normalized(values[0]), (
                f"{name}: tekrarlar deterministik değil: "
                f"{finding_ids(value)} != {ids}"
            )
        if extra_check:
            extra_check(values[0], root)
        print(f"PASS {name}: {ids}")


def write_desktop_config(root: Path, env: dict[str, str], text: str) -> Path:
    config_home = root / "config-home"
    config_dirs = root / "config-dirs"
    config_dir = config_home / "xdg-desktop-portal"
    config_dir.mkdir(parents=True)
    config_dirs.mkdir()
    names = desktops(env) or ["unknown"]
    path = config_dir / f"{names[0]}-portals.conf"
    path.write_text(text, encoding="utf-8")
    env["XDG_CONFIG_HOME"] = str(config_home)
    env["XDG_CONFIG_DIRS"] = str(config_dirs)
    return path


def configure_malformed(root: Path, env: dict[str, str]) -> None:
    write_desktop_config(root, env, "[preferred]\nthis-is-not-a-key-value-line\n")


def configure_missing_backend(root: Path, env: dict[str, str]) -> None:
    write_desktop_config(
        root,
        env,
        "[preferred]\ndefault=definitely-missing-backend;\n",
    )
    data_home = root / "data-home" / "xdg-desktop-portal" / "portals"
    data_home.mkdir(parents=True)
    (data_home / "fake.portal").write_text(
        "[portal]\n"
        "DBusName=org.example.portal.desktop.fake\n"
        "Interfaces=org.freedesktop.impl.portal.Screenshot;\n",
        encoding="utf-8",
    )
    env["XDG_DATA_HOME"] = str(root / "data-home")
    env["XDG_DATA_DIRS"] = str(root / "data-dirs")
    Path(env["XDG_DATA_DIRS"]).mkdir()


def configure_empty_roots(root: Path, env: dict[str, str]) -> None:
    env["XDG_CONFIG_HOME"] = str(root / "config-home")
    env["XDG_CONFIG_DIRS"] = str(root / "config-dirs")
    env["XDG_DATA_HOME"] = str(root / "data-home")
    env["XDG_DATA_DIRS"] = str(root / "data-dirs")
    for key in ("XDG_CONFIG_HOME", "XDG_CONFIG_DIRS", "XDG_DATA_HOME", "XDG_DATA_DIRS"):
        Path(env[key]).mkdir(parents=True)


def configure_runtime_backend(root: Path, env: dict[str, str]) -> None:
    """Create one selected descriptor so the private-bus case is portable."""
    write_desktop_config(root, env, "[preferred]\ndefault=fake;\n")
    data_home = root / "data-home" / "xdg-desktop-portal" / "portals"
    data_home.mkdir(parents=True)
    (data_home / "fake.portal").write_text(
        "[portal]\n"
        "DBusName=org.example.portal.desktop.fake\n"
        "Interfaces=org.freedesktop.impl.portal.Screenshot;\n",
        encoding="utf-8",
    )
    env["XDG_DATA_HOME"] = str(root / "data-home")
    env["XDG_DATA_DIRS"] = str(root / "data-dirs")
    Path(env["XDG_DATA_DIRS"]).mkdir()


def check_cfg002(value: dict, root: Path) -> None:
    finding = next(f for f in value["findings"] if f["id"] == "CFG002")
    selected = value["snapshot"]["portal_config"]["value"]["selected_file"]
    assert str(root) in selected
    assert str(root) in finding["summary"]
    assert finding["severity"] == "warning"
    assert finding["confidence"] == "high"


def check_xdp005(value: dict, root: Path) -> None:
    assert any(f["id"] == "XDP005" for f in value["findings"])
    routes = value["snapshot"]["portal_routes"]["value"]
    screenshot = next(r for r in routes if r["interface"].endswith("Screenshot"))
    assert screenshot["selected_candidates"] == []
    assert "definitely-missing-backend" not in screenshot["selected_candidates"]


def check_xdp003(value: dict, root: Path) -> None:
    assert any(f["id"] == "XDP003" for f in value["findings"])
    assert value["snapshot"]["portal_backends"]["value"] == []


def main() -> int:
    binary = binary_path()
    host_environment = os.environ.copy()
    host_has_target_session = all(
        host_environment.get(key)
        for key in (
            "XDG_CURRENT_DESKTOP",
            "XDG_SESSION_TYPE",
            "WAYLAND_DISPLAY",
        )
    )
    base = fixture_environment(host_environment.copy())
    print(f"binary: {binary}")

    # Parser paths are independent of the host session and must stay stable.
    help_result = subprocess.run(
        [binary, "--help"], env=base, text=True, capture_output=True, timeout=20, check=False
    )
    assert help_result.returncode == 0, help_result.stderr
    invalid_cli = subprocess.run(
        [binary, "--definitely-invalid"],
        env=base,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    assert invalid_cli.returncode == 2, invalid_cli.stderr
    print("PASS parser-exit-codes: help=0 invalid-cli=2")

    # 1. Baseline: text and JSON are both checked. A non-graphical CI runner
    # cannot provide a genuinely healthy portal stack, so only enforce the
    # empty finding set on a host that already exposes the target session.
    text = subprocess.run(
        [binary], env=base, text=True, capture_output=True, timeout=20, check=False
    )
    assert text.returncode in (0, 3), text.stderr
    assert "Findings:" in text.stdout
    baseline_exit = text.returncode
    value, _ = run_json(binary, base, expected_exit=baseline_exit)
    assert f"PortalDoctor {value['portaldoctor_version']}" in text.stdout
    ids = finding_ids(value)
    if ids:
        if host_has_target_session:
            raise AssertionError(f"healthy-baseline findings: {ids}")
        print(f"SKIP healthy-baseline on non-graphical host: {ids}")
    else:
        assert "Findings: none detected." in text.stdout
        print("PASS healthy-baseline: []")

    # 2. Missing desktop identity.
    missing_desktop = base.copy()
    missing_desktop.pop("XDG_CURRENT_DESKTOP", None)
    run_repeated(
        "missing-desktop",
        binary,
        missing_desktop,
        "ENV001",
        expected_exit=baseline_exit,
    )

    # 3. Wayland without compositor socket.
    broken_wayland = base.copy()
    broken_wayland["XDG_SESSION_TYPE"] = "wayland"
    broken_wayland.pop("WAYLAND_DISPLAY", None)
    run_repeated(
        "broken-wayland",
        binary,
        broken_wayland,
        "ENV003",
        expected_exit=3,
    )

    # 4. Malformed desktop-specific portals.conf.
    run_repeated(
        "malformed-portals-conf",
        binary,
        base,
        "CFG002",
        configure_malformed,
        extra_check=check_cfg002,
        expected_exit=baseline_exit,
    )

    # 5. Configured backend descriptor absent, while another fake descriptor exists.
    run_repeated(
        "missing-configured-backend",
        binary,
        base,
        "XDP005",
        configure_missing_backend,
        extra_check=check_xdp005,
        expected_exit=baseline_exit,
    )

    # 6. Empty isolated data/config roots.
    run_repeated(
        "empty-portal-descriptors",
        binary,
        base,
        "XDP003",
        configure_empty_roots,
        extra_check=check_xdp003,
        expected_exit=baseline_exit,
    )

    # 7. Invalid bus address: no session bus and frontend cannot be reached.
    invalid_bus = base.copy()
    with tempfile.TemporaryDirectory(prefix="portaldoctor-bus-") as temp:
        invalid_bus["DBUS_SESSION_BUS_ADDRESS"] = f"unix:path={Path(temp) / 'missing-bus'}"
        run_repeated(
            "invalid-session-bus",
            binary,
            invalid_bus,
            "DBUS001",
            expected_exit=3,
        )
        # The runtime contract also expects the frontend finding; report it if present.
        value, _ = run_json(binary, invalid_bus, expected_exit=3)
        assert_expected_contract(value, "XDP001")

    # 8. Private valid bus with no portal frontend owner.
    dbus_run_session = shutil.which("dbus-run-session")
    if not dbus_run_session:
        raise RuntimeError("dbus-run-session bulunamadı; scenario 8 çalıştırılamadı")
    with tempfile.TemporaryDirectory(prefix="portaldoctor-private-bus-") as temp:
        private_env = base.copy()
        configure_runtime_backend(Path(temp), private_env)
        values = []
        for _ in range(3):
            result = subprocess.run(
                [dbus_run_session, "--", binary, "check", "--json"],
                env=private_env,
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            assert result.returncode == 0, result.stderr
            value = json.loads(result.stdout)
            validate_finding_contract(value)
            values.append(value)
        ids = finding_ids(values[0])
        assert_expected_contract(values[0], "XDP001")
        assert_expected_contract(values[0], "DBUS002")
        assert "DBUS001" not in ids
        text_result = subprocess.run(
            [dbus_run_session, "--", binary, "check"],
            env=private_env,
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
        assert text_result.returncode == 0, text_result.stderr
        xdp001 = next(f for f in values[0]["findings"] if f["id"] == "XDP001")
        assert f"next: {xdp001['recommendation'][0]}" in text_result.stdout
        for value in values[1:]:
            assert normalized(value) == normalized(values[0])
        print(f"PASS private-bus-frontend-absent: {ids}")

    print("E2E fault-injection validation: PASS")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, RuntimeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        raise SystemExit(1)
