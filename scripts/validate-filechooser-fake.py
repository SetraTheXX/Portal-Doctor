#!/usr/bin/env python3
"""Exercise the FileChooser lifecycle against a controlled D-Bus portal.

The harness owns the standard portal bus name inside an isolated
``dbus-run-session`` and implements only the FileChooser/Request methods that
the explicit probe needs. It never creates or reads a real file. The success
fixture returns a synthetic URI only inside the D-Bus response; the product
must discard it and emit no URI or filename.

Examples:
  dbus-run-session -- python3 scripts/validate-filechooser-fake.py \
    --mode success -- target/release/portaldoctor probe filechooser --json
  dbus-run-session -- python3 scripts/validate-filechooser-fake.py \
    --mode request-timeout -- target/release/portaldoctor probe filechooser --json
"""

import argparse
import json
import os
import signal
import subprocess
import sys
import time
import warnings

import gi

warnings.filterwarnings("ignore", category=DeprecationWarning)

gi.require_version("Gio", "2.0")
gi.require_version("GLib", "2.0")
from gi.repository import Gio, GLib


PORTAL_NAME = "org.freedesktop.portal.Desktop"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
FILE_CHOOSER_INTERFACE = "org.freedesktop.portal.FileChooser"
REQUEST_INTERFACE = "org.freedesktop.portal.Request"
INTROSPECTABLE_INTERFACE = "org.freedesktop.DBus.Introspectable"

ROOT_XML = f"""
<node>
  <interface name="{INTROSPECTABLE_INTERFACE}">
    <method name="Introspect">
      <arg name="xml_data" direction="out" type="s"/>
    </method>
  </interface>
  <interface name="{FILE_CHOOSER_INTERFACE}">
    <method name="OpenFile">
      <arg name="parent_window" direction="in" type="s"/>
      <arg name="title" direction="in" type="s"/>
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="handle" direction="out" type="o"/>
    </method>
  </interface>
</node>
"""

UNSUPPORTED_XML = f"""
<node>
  <interface name="{INTROSPECTABLE_INTERFACE}">
    <method name="Introspect">
      <arg name="xml_data" direction="out" type="s"/>
    </method>
  </interface>
</node>
"""

REQUEST_XML = f"""
<node>
  <interface name="{REQUEST_INTERFACE}">
    <method name="Close"/>
    <signal name="Response">
      <arg name="response" type="u"/>
      <arg name="results" type="a{{sv}}"/>
    </signal>
  </interface>
</node>
"""


class FakePortal:
    def __init__(self, mode):
        self.mode = mode
        self.connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.root_info = Gio.DBusNodeInfo.new_for_xml(
            UNSUPPORTED_XML if mode == "unsupported" else ROOT_XML
        )
        self.request_info = Gio.DBusNodeInfo.new_for_xml(REQUEST_XML)
        self.registrations = {}
        self.request_path = None
        self.close_count = 0
        self.root_registrations = []
        if mode != "unavailable":
            self._request_name()
            for interface in self.root_info.interfaces:
                self.root_registrations.append(
                    self.connection.register_object(
                        PORTAL_PATH, interface, self._method_call
                    )
                )

    def _request_name(self):
        result = self.connection.call_sync(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            GLib.Variant("(su)", (PORTAL_NAME, 0x4)),
            GLib.VariantType.new("(u)"),
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        ).unpack()[0]
        if result != 1:
            raise RuntimeError(f"could not own {PORTAL_NAME}: result {result}")

    def _method_call(
        self,
        connection,
        sender,
        object_path,
        interface_name,
        method_name,
        parameters,
        invocation,
    ):
        if interface_name == INTROSPECTABLE_INTERFACE and method_name == "Introspect":
            xml = UNSUPPORTED_XML if self.mode == "unsupported" else ROOT_XML
            invocation.return_value(GLib.Variant("(s)", (xml,)))
            return
        if interface_name == FILE_CHOOSER_INTERFACE and method_name == "OpenFile":
            self._open_file(sender, parameters, invocation)
            return
        invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod", "fake method is not implemented"
        )

    def _open_file(self, sender, parameters, invocation):
        _, _, options = parameters.unpack()
        token_variant = options.get("handle_token")
        token = (
            token_variant.unpack()
            if hasattr(token_variant, "unpack")
            else token_variant
            if token_variant is not None
            else "fake_token"
        )
        sender_component = sender.removeprefix(":").replace(".", "_")
        self.request_path = (
            f"/org/freedesktop/portal/desktop/request/{sender_component}/{token}"
        )
        if self.mode == "transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed", "controlled fake transport failure"
            )
            return
        if self.mode == "request-timeout":
            GLib.timeout_add(6000, self._late_open_file, invocation)
            return
        if self.mode == "late-reply":
            GLib.timeout_add(3500, self._late_open_file, invocation)
            return
        self._register_request()
        invocation.return_value(GLib.Variant("(o)", (self.request_path,)))
        if self.mode in {"success", "close-failure"}:
            GLib.timeout_add(150, self._emit_response, 0, {"uris": ["file:///tmp/fake-selection"]})
        elif self.mode == "cancel":
            # The harness sends SIGINT to the probe; no portal response is
            # emitted, so the product must close the request itself.
            pass
        elif self.mode == "malformed":
            GLib.timeout_add(150, self._emit_response, 0, {})
        elif self.mode == "response-timeout":
            pass

    def _late_open_file(self, invocation):
        if self.request_path is not None:
            self._register_request()
            invocation.return_value(GLib.Variant("(o)", (self.request_path,)))
        return GLib.SOURCE_REMOVE

    def _register_request(self):
        if self.request_path in self.registrations:
            return
        self.registrations[self.request_path] = self.connection.register_object(
            self.request_path, self.request_info.interfaces[0], self._request_method_call
        )

    def _request_method_call(
        self,
        connection,
        sender,
        object_path,
        interface_name,
        method_name,
        parameters,
        invocation,
    ):
        if method_name != "Close":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.UnknownMethod", "fake request method is not implemented"
            )
            return
        self.close_count += 1
        if self.mode == "close-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.AccessDenied", "controlled close failure"
            )
            return
        registration = self.registrations.pop(object_path, None)
        if registration is not None:
            self.connection.unregister_object(registration)
        invocation.return_value(GLib.Variant("()", ()))

    def _emit_response(self, status, results):
        if self.request_path not in self.registrations:
            return GLib.SOURCE_REMOVE
        encoded = {}
        if "uris" in results:
            encoded["uris"] = GLib.Variant("as", results["uris"])
        self.connection.emit_signal(
            None,
            self.request_path,
            REQUEST_INTERFACE,
            "Response",
            GLib.Variant("(ua{sv})", (status, encoded)),
        )
        return GLib.SOURCE_REMOVE

    def close(self):
        for registration in list(self.registrations.values()):
            self.connection.unregister_object(registration)
        for registration in self.root_registrations:
            self.connection.unregister_object(registration)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=(
            "success",
            "close-failure",
            "cancel",
            "malformed",
            "response-timeout",
            "late-reply",
            "request-timeout",
            "transport-failure",
            "unsupported",
        ),
        required=True,
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a PortalDoctor command is required after --")

    fake = FakePortal(args.mode)
    proc = subprocess.Popen(
        command,
        env=os.environ.copy(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    loop = GLib.MainLoop()
    started = time.monotonic()

    def cancel_probe():
        if proc.poll() is None:
            proc.send_signal(signal.SIGINT)
        return GLib.SOURCE_REMOVE

    def poll_process():
        if proc.poll() is not None:
            loop.quit()
            return GLib.SOURCE_REMOVE
        if time.monotonic() - started > 45:
            proc.kill()
            loop.quit()
            return GLib.SOURCE_REMOVE
        return GLib.SOURCE_CONTINUE

    if args.mode == "cancel":
        GLib.timeout_add(300, cancel_probe)
    GLib.timeout_add(50, poll_process)
    loop.run()
    stdout, stderr = proc.communicate()
    fake.close()

    if proc.returncode is None:
        raise AssertionError("fake portal harness did not observe process completion")
    if "file://" in stdout or "fake-selection" in stdout or "file://" in stderr:
        raise AssertionError("FileChooser output leaked the synthetic URI")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise AssertionError(f"expected JSON output, got {stdout!r}; stderr={stderr!r}") from error

    expected = {
        "success": ("success", "completed", 0),
        "close-failure": ("success", "failed", 1),
        "cancel": ("user_cancelled", "completed", 1),
        "malformed": ("malformed_response", "completed", 1),
        "response-timeout": ("timed_out", "completed", 1),
        "late-reply": ("timed_out", "completed", 1),
        "request-timeout": ("timed_out", "unverified", 1),
        "transport-failure": ("infrastructure_failure", "unverified", 1),
        "unsupported": ("unsupported", "not_required", 1),
    }[args.mode]
    actual = (result["status"], result["cleanup"]["status"], proc.returncode)
    if actual != expected:
        raise AssertionError(f"{args.mode}: expected {expected}, got {actual}; stderr={stderr!r}")
    if args.mode in {
        "success",
        "close-failure",
        "cancel",
        "malformed",
        "response-timeout",
        "late-reply",
    } and fake.close_count != 1:
        raise AssertionError(f"{args.mode}: Request.Close was not observed")
    if args.mode in {"request-timeout", "transport-failure", "unsupported"} and fake.close_count != 0:
        raise AssertionError(
            f"{args.mode}: Request.Close was observed although no request object was created"
        )
    print(json.dumps({"mode": args.mode, "result": result, "close_calls": fake.close_count}, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fake FileChooser validation failed: {error}", file=sys.stderr)
        raise
