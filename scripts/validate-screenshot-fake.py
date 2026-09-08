#!/usr/bin/env python3
"""Exercise the bounded Screenshot lifecycle against a controlled portal.

The fake returns a synthetic portal URI only inside the D-Bus response. The
product must classify its type and discard it without printing, serializing or
persisting the URI or image metadata.
"""

import argparse
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import warnings

import gi

warnings.filterwarnings("ignore", category=DeprecationWarning)

gi.require_version("Gio", "2.0")
gi.require_version("GLib", "2.0")
from gi.repository import Gio, GLib


PORTAL_NAME = "org.freedesktop.portal.Desktop"
GNOME_BACKEND_NAME = "org.freedesktop.impl.portal.desktop.gnome"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
SCREENSHOT_INTERFACE = "org.freedesktop.portal.Screenshot"
PROPERTIES_INTERFACE = "org.freedesktop.DBus.Properties"
REQUEST_INTERFACE = "org.freedesktop.portal.Request"
INTROSPECTABLE_INTERFACE = "org.freedesktop.DBus.Introspectable"

def root_xml(include_targets):
    """Return the exact capability shape exposed by the selected fake."""
    available_targets = (
        '    <property type="u" name="AvailableTargets" access="read"/>\n'
        if include_targets
        else ""
    )
    return f"""
<node>
  <interface name="{INTROSPECTABLE_INTERFACE}">
    <method name="Introspect">
      <arg name="xml_data" direction="out" type="s"/>
    </method>
  </interface>
  <interface name="{PROPERTIES_INTERFACE}">
    <method name="Get">
      <arg name="interface_name" direction="in" type="s"/>
      <arg name="property_name" direction="in" type="s"/>
      <arg name="value" direction="out" type="v"/>
    </method>
    <method name="GetAll">
      <arg name="interface_name" direction="in" type="s"/>
      <arg name="values" direction="out" type="a{{sv}}"/>
    </method>
  </interface>
  <interface name="{SCREENSHOT_INTERFACE}">
    <property type="u" name="version" access="read"/>
{available_targets}    <method name="Screenshot">
      <arg name="parent_window" direction="in" type="s"/>
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="handle" direction="out" type="o"/>
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
        self.base_mode = mode.removeprefix("v2-")
        self.is_v2 = mode.startswith("v2-")
        self.trusted_v2 = self.is_v2 and self.base_mode != "untrusted"
        self.connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.root_info = Gio.DBusNodeInfo.new_for_xml(root_xml(not self.is_v2))
        self.request_info = Gio.DBusNodeInfo.new_for_xml(REQUEST_XML)
        self.registrations = {}
        self.request_path = None
        self.close_count = 0
        self.request_active = False
        self.response_emitted = False
        self.unexpected_close = False
        self.received_options = None
        self.root_registrations = []
        self.child_env = os.environ.copy()
        if self.is_v2:
            self._prepare_v2_environment()
        self._request_name()
        for interface in self.root_info.interfaces:
            self.root_registrations.append(
                self.connection.register_object(
                    PORTAL_PATH, interface, self._method_call
                )
            )

    def _prepare_v2_environment(self):
        """Provide route evidence without invoking a backend operation."""
        self.child_env.update(
            {
                "XDG_CURRENT_DESKTOP": "GNOME",
                "XDG_SESSION_DESKTOP": "gnome",
                "XDG_SESSION_TYPE": "wayland",
                "WAYLAND_DISPLAY": "wayland-0",
            }
        )
        if not self.trusted_v2:
            return

        self.route_root = tempfile.TemporaryDirectory(prefix="portaldoctor-v2-route-")
        root = os.path.abspath(self.route_root.name)
        config_home = os.path.join(root, "config")
        data_home = os.path.join(root, "data")
        config_path = os.path.join(
            config_home, "xdg-desktop-portal", "gnome-portals.conf"
        )
        descriptor_path = os.path.join(
            data_home, "xdg-desktop-portal", "portals", "gnome.portal"
        )
        os.makedirs(os.path.dirname(config_path), exist_ok=True)
        os.makedirs(os.path.dirname(descriptor_path), exist_ok=True)
        with open(config_path, "w", encoding="utf-8") as config:
            config.write("[preferred]\ndefault=gnome\n")
        with open(descriptor_path, "w", encoding="utf-8") as descriptor:
            descriptor.write(
                "[portal]\n"
                f"DBusName={GNOME_BACKEND_NAME}\n"
                f"Interfaces={self._screenshot_interface_name()}\n"
                "UseIn=gnome\n"
            )
        self.child_env.update(
            {
                "XDG_CONFIG_HOME": config_home,
                "XDG_CONFIG_DIRS": os.path.join(root, "empty-config"),
                "XDG_DATA_HOME": data_home,
                "XDG_DATA_DIRS": os.path.join(root, "empty-data"),
            }
        )

    @staticmethod
    def _screenshot_interface_name():
        return "org.freedesktop.impl.portal.Screenshot"

    def _request_name(self):
        names = [PORTAL_NAME]
        if self.trusted_v2:
            names.append(GNOME_BACKEND_NAME)
        for name in names:
            result = self.connection.call_sync(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "RequestName",
                GLib.Variant("(su)", (name, 0x4)),
                GLib.VariantType.new("(u)"),
                Gio.DBusCallFlags.NONE,
                -1,
                None,
            ).unpack()[0]
            if result != 1:
                raise RuntimeError(f"could not own {name}: result {result}")

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
        if (
            self.base_mode == "unavailable"
            and interface_name == INTROSPECTABLE_INTERFACE
            and method_name == "Introspect"
        ):
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.ServiceUnknown",
                "controlled unavailable portal",
            )
            return
        if interface_name == INTROSPECTABLE_INTERFACE and method_name == "Introspect":
            invocation.return_value(GLib.Variant("(s)", (root_xml(not self.is_v2),)))
            return
        if interface_name == PROPERTIES_INTERFACE and method_name == "Get":
            self._get_property(parameters, invocation)
            return
        if interface_name == PROPERTIES_INTERFACE and method_name == "GetAll":
            self._get_all_properties(invocation)
            return
        if interface_name == SCREENSHOT_INTERFACE and method_name == "Screenshot":
            self._screenshot(sender, parameters, invocation)
            return
        invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            "fake method is not implemented",
        )

    def _get_property(self, parameters, invocation):
        _, property_name = parameters.unpack()
        if property_name == "version":
            value = 2 if self.is_v2 or self.base_mode == "unsupported-version" else 3
        elif property_name == "AvailableTargets" and not self.is_v2:
            value = 1 if self.base_mode == "unsupported-target" else 2
        else:
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.UnknownProperty",
                "fake property is not implemented",
            )
            return
        invocation.return_value(GLib.Variant("(v)", (GLib.Variant("u", value),)))

    def _get_all_properties(self, invocation):
        version = 2 if self.is_v2 or self.base_mode == "unsupported-version" else 3
        values = {
            "version": GLib.Variant("u", version),
        }
        if not self.is_v2:
            targets = 1 if self.base_mode == "unsupported-target" else 2
            values["AvailableTargets"] = GLib.Variant("u", targets)
        invocation.return_value(GLib.Variant("(a{sv})", (values,)))

    def _screenshot(self, sender, parameters, invocation):
        _, options = parameters.unpack()
        self.received_options = options
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
        if self.base_mode == "transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "controlled fake transport failure",
            )
            return
        if self.base_mode in {"request-timeout", "request-cancel"}:
            GLib.timeout_add(6000, self._late_screenshot, invocation)
            return
        if self.base_mode == "late-reply":
            GLib.timeout_add(3500, self._late_screenshot, invocation)
            return

        self._register_request()
        invocation.return_value(GLib.Variant("(o)", (self.request_path,)))
        if self.base_mode == "success":
            GLib.timeout_add(150, self._emit_response, 0, {"uri": "document://portal/fake-screenshot.png"})
        elif self.base_mode == "malformed":
            GLib.timeout_add(150, self._emit_response, 0, {})
        elif self.base_mode == "malformed-type":
            GLib.timeout_add(
                150,
                self._emit_response,
                0,
                {"uri": GLib.Variant("u", 7)},
            )
        elif self.base_mode == "portal-failure":
            GLib.timeout_add(150, self._emit_response, 2, {})
        elif self.base_mode in {"cancel", "close-failure"}:
            # The harness sends SIGINT while the request is active.
            pass
        elif self.base_mode == "response-timeout":
            pass

    def _late_screenshot(self, invocation):
        if self.request_path is not None:
            self._register_request()
            invocation.return_value(GLib.Variant("(o)", (self.request_path,)))
        return GLib.SOURCE_REMOVE

    def _register_request(self):
        if self.request_path in self.registrations:
            return
        self.request_active = True
        self.registrations[self.request_path] = self.connection.register_object(
            self.request_path,
            self.request_info.interfaces[0],
            self._request_method_call,
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
                "org.freedesktop.DBus.Error.UnknownMethod",
                "fake request method is not implemented",
            )
            return
        self.close_count += 1
        if self.response_emitted:
            self.unexpected_close = True
            invocation.return_dbus_error(
                "org.freedesktop.portal.Error.UnexpectedCloseAfterResponse",
                "Request.Close is invalid after Response",
            )
            return
        if self.base_mode == "close-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.AccessDenied",
                "controlled close failure",
            )
            return
        registration = self.registrations.pop(object_path, None)
        if registration is not None:
            self.connection.unregister_object(registration)
        self.request_active = False
        invocation.return_value(GLib.Variant("()", ()))

    def _emit_response(self, status, results):
        if self.request_path not in self.registrations:
            return GLib.SOURCE_REMOVE
        self.response_emitted = True
        self.request_active = False
        # Keep a sentinel registration only so a product-side Close after a
        # terminal Response is observable and fails the harness. The request
        # itself is no longer active after this signal.
        encoded = {}
        for key, value in results.items():
            encoded[key] = value if isinstance(value, GLib.Variant) else GLib.Variant("s", value)
        self.connection.emit_signal(
            None,
            self.request_path,
            REQUEST_INTERFACE,
            "Response",
            GLib.Variant("(ua{sv})", (status, encoded)),
        )
        return GLib.SOURCE_REMOVE

    def validate_options(self):
        """Check capability-specific options without printing the token."""
        if self.received_options is None:
            return

        def value(name):
            option = self.received_options.get(name)
            if option is None:
                raise AssertionError(f"missing Screenshot option {name!r}")
            return option.unpack() if hasattr(option, "unpack") else option

        token = value("handle_token")
        if not isinstance(token, str) or not token:
            raise AssertionError("handle_token was not a non-empty string")
        if value("modal") is not True or value("interactive") is not True:
            raise AssertionError("Screenshot did not receive modal=true and interactive=true")
        if self.is_v2:
            if "target" in self.received_options:
                raise AssertionError("v2 Screenshot request unexpectedly received target")
        elif value("target") != 2:
            raise AssertionError("v3 Screenshot request did not receive target=2")

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
            "malformed-type",
            "portal-failure",
            "response-timeout",
            "late-reply",
            "request-timeout",
            "transport-failure",
            "unsupported-version",
            "unsupported-target",
            "unavailable",
            "v2-success",
            "v2-close-failure",
            "v2-cancel",
            "v2-malformed",
            "v2-malformed-type",
            "v2-portal-failure",
            "v2-response-timeout",
            "v2-late-reply",
            "v2-request-timeout",
            "v2-request-cancel",
            "v2-transport-failure",
            "v2-untrusted",
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
        env=fake.child_env,
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

    if fake.base_mode in {"cancel", "close-failure", "request-cancel"}:
        GLib.timeout_add(300, cancel_probe)
    GLib.timeout_add(50, poll_process)
    loop.run()
    stdout, stderr = proc.communicate()
    fake.validate_options()
    if fake.unexpected_close:
        raise AssertionError("Request.Close was called after a terminal Response")
    open_requests = int(fake.request_active)
    fake.close()

    if proc.returncode is None:
        raise AssertionError("fake portal harness did not observe process completion")
    for forbidden in (
        "document://",
        "file://",
        "fake-screenshot",
        ".png",
        "window-secret",
    ):
        if forbidden in stdout or forbidden in stderr:
            raise AssertionError(f"Screenshot output leaked {forbidden!r}")
    if fake.is_v2 and fake.trusted_v2:
        if "GNOME portal UI" not in stderr or "screen, window, or area" not in stderr:
            raise AssertionError("v2 warning did not disclose the portal-selected target scope")
    if not fake.is_v2 and fake.base_mode not in {"unsupported-version", "unsupported-target", "unavailable"}:
        if "choose a window" not in stderr:
            raise AssertionError("v3 warning did not disclose the Window target")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise AssertionError(f"expected JSON output, got {stdout!r}; stderr={stderr!r}") from error

    expected = {
        "success": ("success", "not_required", 0, 0, 0),
        "close-failure": ("user_cancelled", "failed", 1, 1, 1),
        "cancel": ("user_cancelled", "completed", 1, 1, 0),
        "malformed": ("malformed_response", "not_required", 1, 0, 0),
        "malformed-type": ("malformed_response", "not_required", 1, 0, 0),
        "portal-failure": ("infrastructure_failure", "not_required", 1, 0, 0),
        "response-timeout": ("timed_out", "completed", 1, 1, 0),
        "late-reply": ("timed_out", "completed", 1, 1, 0),
        "request-timeout": ("timed_out", "unverified", 1, 0, 0),
        "transport-failure": ("infrastructure_failure", "unverified", 1, 0, 0),
        "unsupported-version": ("unsupported", "not_required", 1, 0, 0),
        "unsupported-target": ("unsupported", "not_required", 1, 0, 0),
        "unavailable": ("unavailable", "not_required", 1, 0, 0),
        "v2-success": ("success", "not_required", 0, 0, 0),
        "v2-close-failure": ("user_cancelled", "failed", 1, 1, 1),
        "v2-cancel": ("user_cancelled", "completed", 1, 1, 0),
        "v2-malformed": ("malformed_response", "not_required", 1, 0, 0),
        "v2-malformed-type": ("malformed_response", "not_required", 1, 0, 0),
        "v2-portal-failure": ("infrastructure_failure", "not_required", 1, 0, 0),
        "v2-response-timeout": ("timed_out", "completed", 1, 1, 0),
        "v2-late-reply": ("timed_out", "completed", 1, 1, 0),
        "v2-request-timeout": ("timed_out", "unverified", 1, 0, 0),
        "v2-request-cancel": ("user_cancelled", "unverified", 1, 0, 0),
        "v2-transport-failure": ("infrastructure_failure", "unverified", 1, 0, 0),
        "v2-untrusted": ("unsupported", "not_required", 1, 0, 0),
    }[args.mode]
    actual = (
        result["status"],
        result["cleanup"]["status"],
        proc.returncode,
        fake.close_count,
        open_requests,
    )
    if actual != expected:
        raise AssertionError(f"{args.mode}: expected {expected}, got {actual}; stderr={stderr!r}")
    print(
        json.dumps(
            {
                "mode": args.mode,
                "result": result,
                "close_calls": fake.close_count,
                "open_requests_after_probe": open_requests,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fake Screenshot validation failed: {error}", file=sys.stderr)
        raise
