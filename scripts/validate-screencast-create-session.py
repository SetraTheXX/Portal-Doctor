#!/usr/bin/env python3
"""Controlled ScreenCast CreateSession portal for the internal lifecycle tests.

The fake deliberately keeps Request and Session registrations observable after
terminal responses. A mistaken Request.Close after Response therefore becomes
an explicit harness failure. State contains only counters and booleans; no
handle token, object path, or raw D-Bus payload is persisted.
"""

import argparse
import json
import os
import sys
import tempfile
import warnings

import gi

warnings.filterwarnings("ignore", category=DeprecationWarning)

gi.require_version("Gio", "2.0")
gi.require_version("GLib", "2.0")
from gi.repository import Gio, GLib


PORTAL_NAME = "org.freedesktop.portal.Desktop"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
SCREENCAST_INTERFACE = "org.freedesktop.portal.ScreenCast"
REQUEST_INTERFACE = "org.freedesktop.portal.Request"
SESSION_INTERFACE = "org.freedesktop.portal.Session"
INTROSPECTABLE_INTERFACE = "org.freedesktop.DBus.Introspectable"


ROOT_XML = f"""
<node>
  <interface name="{INTROSPECTABLE_INTERFACE}">
    <method name="Introspect">
      <arg name="xml_data" direction="out" type="s"/>
    </method>
  </interface>
  <interface name="{SCREENCAST_INTERFACE}">
    <method name="CreateSession">
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

SESSION_XML = f"""
<node>
  <interface name="{SESSION_INTERFACE}">
    <method name="Close"/>
  </interface>
</node>
"""


EXPECTED_SESSION_MODES = {
    "malformed-missing-session-present",
    "malformed-wrong-type-session-present",
    "malformed-path-session-present",
    "malformed-signal-session-present",
    "malformed-expected-session-close-failure",
}


class FakePortal:
    def __init__(self, mode, state_file):
        self.mode = mode
        self.state_file = state_file
        self.connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.root_info = Gio.DBusNodeInfo.new_for_xml(
            UNSUPPORTED_XML if mode == "unsupported" else ROOT_XML
        )
        self.request_info = Gio.DBusNodeInfo.new_for_xml(REQUEST_XML)
        self.session_info = Gio.DBusNodeInfo.new_for_xml(SESSION_XML)
        self.root_registrations = []
        self.request_registrations = {}
        self.session_registrations = {}
        self.request_path = None
        self.session_path = None
        self.request_close_calls = 0
        self.session_close_calls = 0
        self.request_active = False
        self.session_active = False
        self.response_emitted = False
        self.unexpected_close = False
        self.tokens_valid = True

        self._request_name()
        for interface in self.root_info.interfaces:
            self.root_registrations.append(
                self.connection.register_object(
                    PORTAL_PATH, interface, self._method_call
                )
            )

        self._write_state(ready=True)

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

    def _write_state(self, ready=False):
        state = {
            "ready": bool(ready),
            "request_close_calls": self.request_close_calls,
            "session_close_calls": self.session_close_calls,
            "request_active": self.request_active,
            "session_active": self.session_active,
            "unexpected_close": self.unexpected_close,
            "tokens_valid": self.tokens_valid,
        }
        directory = os.path.dirname(os.path.abspath(self.state_file))
        fd, temporary = tempfile.mkstemp(
            prefix="portaldoctor-screencast-state-", dir=directory, text=True
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as output:
                json.dump(state, output, sort_keys=True)
                output.write("\n")
            os.replace(temporary, self.state_file)
        finally:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass

    @staticmethod
    def _string_option(options, name):
        value = options.get(name)
        if value is None:
            return None
        try:
            value = value.unpack()
        except AttributeError:
            pass
        return value if isinstance(value, str) else None

    @staticmethod
    def _sender_component(sender):
        return sender.removeprefix(":").replace(".", "_")

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
            self.mode == "unavailable"
            and interface_name == INTROSPECTABLE_INTERFACE
            and method_name == "Introspect"
        ):
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.ServiceUnknown",
                "controlled unavailable portal",
            )
            return
        if interface_name == INTROSPECTABLE_INTERFACE and method_name == "Introspect":
            xml = UNSUPPORTED_XML if self.mode == "unsupported" else ROOT_XML
            invocation.return_value(GLib.Variant("(s)", (xml,)))
            return
        if interface_name == SCREENCAST_INTERFACE and method_name == "CreateSession":
            self._create_session(sender, parameters, invocation)
            return
        invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            "controlled fake method is not implemented",
        )

    def _create_session(self, sender, parameters, invocation):
        (options,) = parameters.unpack()
        request_token = self._string_option(options, "handle_token")
        session_token = self._string_option(options, "session_handle_token")
        self.tokens_valid = (
            request_token is not None
            and session_token is not None
            and request_token != session_token
        )
        sender_component = self._sender_component(sender)
        if request_token is None:
            request_token = "invalid"
        if session_token is None:
            session_token = "invalid"
        self.request_path = (
            f"/org/freedesktop/portal/desktop/request/{sender_component}/{request_token}"
        )
        self.session_path = (
            f"/org/freedesktop/portal/desktop/session/{sender_component}/{session_token}"
        )

        if self.mode == "unanswered":
            # No Request object and no method reply: the client may only make
            # a best-effort prediction and must report ownership unverified.
            return
        if self.mode == "request-timeout":
            self._register_request()
            GLib.timeout_add(250, self._return_request, invocation)
            return
        if self.mode == "late-reply":
            self._register_request()
            GLib.timeout_add(70, self._return_request, invocation)
            return

        self._register_request()
        if self.mode == "transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "controlled fake transport failure",
            )
            return
        invocation.return_value(GLib.Variant("(o)", (self.request_path,)))

        if self.mode in EXPECTED_SESSION_MODES:
            self._register_session()

        if self.mode == "success":
            self._register_session()
            GLib.timeout_add(10, self._emit_response, 0, "success")
        elif self.mode == "portal-cancel":
            GLib.timeout_add(10, self._emit_response, 1, "cancel")
        elif self.mode == "portal-failure":
            GLib.timeout_add(10, self._emit_response, 2, "failure")
        elif self.mode == "malformed-missing":
            GLib.timeout_add(10, self._emit_response, 0, "missing")
        elif self.mode == "malformed-wrong-type":
            GLib.timeout_add(10, self._emit_response, 0, "wrong-type")
        elif self.mode == "malformed-path":
            GLib.timeout_add(10, self._emit_response, 0, "wrong-path")
        elif self.mode == "malformed-signal":
            GLib.timeout_add(10, self._emit_malformed_signal)
        elif self.mode == "malformed-missing-session-present":
            GLib.timeout_add(10, self._emit_response, 0, "missing")
        elif self.mode == "malformed-wrong-type-session-present":
            GLib.timeout_add(10, self._emit_response, 0, "wrong-type")
        elif self.mode == "malformed-path-session-present":
            GLib.timeout_add(10, self._emit_response, 0, "wrong-path")
        elif self.mode == "malformed-signal-session-present":
            GLib.timeout_add(10, self._emit_malformed_signal)
        elif self.mode == "malformed-expected-session-close-failure":
            GLib.timeout_add(10, self._emit_response, 0, "missing")
        elif self.mode == "response-timeout":
            pass
        elif self.mode in {"client-cancel", "request-close-failure"}:
            pass
        elif self.mode == "session-close-failure":
            self._register_session()
            GLib.timeout_add(10, self._emit_response, 0, "success")
        elif self.mode == "session-close-ambiguous":
            self._register_session()
            GLib.timeout_add(10, self._emit_response, 0, "success")

    def _return_request(self, invocation):
        if self.request_path is not None:
            invocation.return_value(GLib.Variant("(o)", (self.request_path,)))
        return GLib.SOURCE_REMOVE

    def _register_request(self):
        if self.request_path in self.request_registrations:
            return
        self.request_active = True
        self.request_registrations[self.request_path] = self.connection.register_object(
            self.request_path,
            self.request_info.interfaces[0],
            self._request_method_call,
        )
        self._write_state()

    def _register_session(self):
        if self.session_path in self.session_registrations:
            return
        self.session_active = True
        self.session_registrations[self.session_path] = self.connection.register_object(
            self.session_path,
            self.session_info.interfaces[0],
            self._session_method_call,
        )
        self._write_state()

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
                "controlled request method is not implemented",
            )
            return
        self.request_close_calls += 1
        if self.response_emitted:
            self.unexpected_close = True
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.portal.Error.UnexpectedCloseAfterResponse",
                "Request.Close is invalid after Response",
            )
            return
        if self.mode == "request-close-failure":
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.AccessDenied",
                "controlled request close failure",
            )
            return
        registration = self.request_registrations.pop(object_path, None)
        if registration is not None:
            self.connection.unregister_object(registration)
        self.request_active = False
        self._write_state()
        invocation.return_value(GLib.Variant("()", ()))

    def _session_method_call(
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
                "controlled session method is not implemented",
            )
            return
        self.session_close_calls += 1
        if self.mode in {
            "session-close-failure",
            "malformed-expected-session-close-failure",
        }:
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.AccessDenied",
                "controlled session close failure",
            )
            return
        if self.mode == "session-close-ambiguous":
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.UnknownObject",
                "controlled ambiguous session close",
            )
            return
        registration = self.session_registrations.pop(object_path, None)
        if registration is not None:
            self.connection.unregister_object(registration)
        self.session_active = False
        self._write_state()
        invocation.return_value(GLib.Variant("()", ()))

    def _emit_response(self, status, kind):
        if self.request_path not in self.request_registrations:
            return GLib.SOURCE_REMOVE
        self.response_emitted = True
        self.request_active = False
        results = {}
        if kind == "success":
            results["session_handle"] = GLib.Variant("s", self.session_path)
        elif kind == "wrong-type":
            results["session_handle"] = GLib.Variant("u", 7)
        elif kind == "wrong-path":
            results["session_handle"] = GLib.Variant(
                "s", "/org/freedesktop/portal/desktop/session/9_999/foreign"
            )
        self._write_state()
        self.connection.emit_signal(
            None,
            self.request_path,
            REQUEST_INTERFACE,
            "Response",
            GLib.Variant("(ua{sv})", (status, results)),
        )
        return GLib.SOURCE_REMOVE

    def _emit_malformed_signal(self):
        if self.request_path not in self.request_registrations:
            return GLib.SOURCE_REMOVE
        self.response_emitted = True
        self.request_active = False
        self._write_state()
        self.connection.emit_signal(
            None,
            self.request_path,
            REQUEST_INTERFACE,
            "Response",
            GLib.Variant("(sa{sv})", ("wrong-signature", {})),
        )
        return GLib.SOURCE_REMOVE

    def close(self):
        for registration in list(self.request_registrations.values()):
            self.connection.unregister_object(registration)
        for registration in list(self.session_registrations.values()):
            self.connection.unregister_object(registration)
        for registration in self.root_registrations:
            self.connection.unregister_object(registration)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", required=True)
    parser.add_argument("--state-file", required=True)
    args = parser.parse_args()

    fake = FakePortal(args.mode, args.state_file)
    loop = GLib.MainLoop()

    def stop(_signum, _frame):
        loop.quit()

    import signal

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    loop.run()
    fake.close()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fake ScreenCast validation failed: {error}", file=sys.stderr)
        raise
