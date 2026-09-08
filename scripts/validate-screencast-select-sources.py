#!/usr/bin/env python3
"""Controlled ScreenCast portal for the bounded ScreenCast matrices.

The fake keeps terminal Request objects registered as spies.  A subsequent
Request.Close is therefore observable as an explicit protocol failure.  The
state file contains counters and booleans only; tokens, object paths and raw
portal payloads are never persisted.
"""

import argparse
import json
import os
import signal
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
PROPERTIES_INTERFACE = "org.freedesktop.DBus.Properties"
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
    <property name="AvailableSourceTypes" type="u" access="read"/>
    <method name="CreateSession">
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="handle" direction="out" type="o"/>
    </method>
    <method name="SelectSources">
      <arg name="session_handle" direction="in" type="o"/>
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="handle" direction="out" type="o"/>
    </method>
    <method name="Start">
      <arg name="session_handle" direction="in" type="o"/>
      <arg name="parent_window" direction="in" type="s"/>
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="handle" direction="out" type="o"/>
    </method>
    <method name="OpenPipeWireRemote">
      <arg name="session_handle" direction="in" type="o"/>
      <arg name="options" direction="in" type="a{{sv}}"/>
      <arg name="fd" direction="out" type="h"/>
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
</node>
"""

ROOT_XML_NO_PROPERTY = ROOT_XML.replace(
    '    <property name="AvailableSourceTypes" type="u" access="read"/>\n', ""
)

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


class FakePortal:
    def __init__(self, mode, state_file, fd_marker=None):
        self.mode = mode
        self.state_file = state_file
        self.fd_marker = fd_marker
        self.connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        root_xml = (
            ROOT_XML_NO_PROPERTY
            if mode == "select-unsupported-property"
            else ROOT_XML
        )
        self.root_xml = root_xml
        self.root_info = Gio.DBusNodeInfo.new_for_xml(root_xml)
        self.request_info = Gio.DBusNodeInfo.new_for_xml(REQUEST_XML)
        self.session_info = Gio.DBusNodeInfo.new_for_xml(SESSION_XML)
        self.root_registrations = []
        self.request_registrations = {}
        self.request_terminal = {}
        self.session_registrations = {}
        self.create_request_path = None
        self.select_request_path = None
        self.start_request_path = None
        self.current_request_path = None
        self.session_path = None
        self.session_token = None
        self.select_token = None
        self.request_close_calls = 0
        self.create_request_close_calls = 0
        self.select_request_close_calls = 0
        self.start_request_close_calls = 0
        self.create_calls = 0
        self.select_calls = 0
        self.start_calls = 0
        self.open_calls = 0
        self.session_close_calls = 0
        self.request_active = False
        self.select_request_active = False
        self.start_request_active = False
        self.session_active = False
        self.unexpected_close = False
        self.tokens_valid = True
        self.select_options_valid = True
        self.start_options_valid = True
        self.open_options_valid = True
        self.open_fd_sent = 0
        self.fd_close_before_session = False
        self.start_streams_payload_emitted = False
        self.property_get_calls = 0
        self.property_request_valid = True

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
            "create_request_close_calls": self.create_request_close_calls,
            "select_request_close_calls": self.select_request_close_calls,
            "start_request_close_calls": self.start_request_close_calls,
            "create_calls": self.create_calls,
            "select_calls": self.select_calls,
            "start_calls": self.start_calls,
            "open_calls": self.open_calls,
            "session_close_calls": self.session_close_calls,
            "request_active": self.request_active,
            "select_request_active": self.select_request_active,
            "start_request_active": self.start_request_active,
            "session_active": self.session_active,
            "unexpected_close": self.unexpected_close,
            "tokens_valid": self.tokens_valid,
            "select_options_valid": self.select_options_valid,
            "start_options_valid": self.start_options_valid,
            "open_options_valid": self.open_options_valid,
            "open_fd_sent": self.open_fd_sent,
            "fd_close_before_session": self.fd_close_before_session,
            "start_streams_payload_emitted": self.start_streams_payload_emitted,
            "property_get_calls": self.property_get_calls,
            "property_request_valid": self.property_request_valid,
        }
        directory = os.path.dirname(os.path.abspath(self.state_file))
        fd, temporary = tempfile.mkstemp(
            prefix="portaldoctor-screencast-select-state-", dir=directory, text=True
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
    def _uint_option(options, name):
        value = options.get(name)
        if value is None:
            return None
        try:
            value = value.unpack()
        except AttributeError:
            pass
        return value if isinstance(value, int) and value >= 0 else None

    @staticmethod
    def _bool_option(options, name):
        value = options.get(name)
        if value is None:
            return None
        try:
            value = value.unpack()
        except AttributeError:
            pass
        return value if isinstance(value, bool) else None

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
        if interface_name == INTROSPECTABLE_INTERFACE and method_name == "Introspect":
            if self.mode == "select-unavailable":
                invocation.return_dbus_error(
                    "org.freedesktop.DBus.Error.ServiceUnknown",
                    "controlled unavailable ScreenCast portal",
                )
                return
            invocation.return_value(GLib.Variant("(s)", (self.root_xml,)))
            return
        if interface_name == PROPERTIES_INTERFACE and method_name == "Get":
            self._get_property(parameters, invocation)
            return
        if interface_name == PROPERTIES_INTERFACE and method_name == "GetAll":
            self._get_all_properties(invocation)
            return
        if interface_name == SCREENCAST_INTERFACE and method_name == "CreateSession":
            self._create_session(sender, parameters, invocation)
            return
        if interface_name == SCREENCAST_INTERFACE and method_name == "SelectSources":
            self._select_sources(sender, parameters, invocation)
            return
        if interface_name == SCREENCAST_INTERFACE and method_name == "Start":
            self._start(sender, parameters, invocation)
            return
        if (
            interface_name == SCREENCAST_INTERFACE
            and method_name == "OpenPipeWireRemote"
        ):
            self._open_pipe_wire_remote(parameters, invocation)
            return
        invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            "controlled fake method is not implemented",
        )

    def _get_property(self, parameters, invocation):
        interface_name, property_name = parameters.unpack()
        self.property_get_calls += 1
        self.property_request_valid &= (
            interface_name == SCREENCAST_INTERFACE
            and property_name == "AvailableSourceTypes"
        )
        self._write_state()
        if (
            interface_name != SCREENCAST_INTERFACE
            or property_name != "AvailableSourceTypes"
            or self.mode == "select-unsupported-property"
        ):
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.UnknownProperty",
                "controlled property is not available",
            )
            return
        value = 1 if self.mode == "select-unsupported-window" else 2
        invocation.return_value(GLib.Variant("(v)", (GLib.Variant("u", value),)))

    def _get_all_properties(self, invocation):
        value = 1 if self.mode == "select-unsupported-window" else 2
        invocation.return_value(
            GLib.Variant(
                "(a{sv})",
                ({"AvailableSourceTypes": GLib.Variant("u", value)},),
            )
        )

    def _create_session(self, sender, parameters, invocation):
        self.create_calls += 1
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
        self.session_token = session_token
        self.create_request_path = (
            f"/org/freedesktop/portal/desktop/request/{sender_component}/{request_token}"
        )
        self.current_request_path = self.create_request_path
        self.session_path = (
            f"/org/freedesktop/portal/desktop/session/{sender_component}/{session_token}"
        )
        self._register_request()
        if self.mode == "aggregate-create-failure":
            invocation.return_value(GLib.Variant("(o)", (self.create_request_path,)))
            GLib.timeout_add(10, self._emit_response, 2, "empty")
            return
        self._register_session()
        invocation.return_value(GLib.Variant("(o)", (self.create_request_path,)))
        GLib.timeout_add(10, self._emit_response, 0, "success")

    def _register_request(self):
        path = self.current_request_path
        if path is None or path in self.request_registrations:
            return
        self.request_active = True
        if path == self.select_request_path:
            self.select_request_active = True
        if path == self.start_request_path:
            self.start_request_active = True
        self.request_terminal[path] = False
        self.request_registrations[path] = self.connection.register_object(
            path,
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

    def _select_sources(self, sender, parameters, invocation):
        session_handle, options = parameters.unpack()
        self.select_calls += 1
        request_token = self._string_option(options, "handle_token")
        self.select_options_valid &= (
            set(options.keys()) == {"handle_token", "types", "multiple"}
            and session_handle == self.session_path
            and request_token is not None
            and request_token != self.session_token
            and self._uint_option(options, "types") == 2
            and self._bool_option(options, "multiple") is False
        )
        sender_component = self._sender_component(sender)
        if request_token is None:
            request_token = "invalid"
        self.select_request_path = (
            f"/org/freedesktop/portal/desktop/request/{sender_component}/{request_token}"
        )
        self.select_token = request_token
        self.current_request_path = self.select_request_path
        if self.mode in {"select-unanswered", "select-request-cancel"}:
            if self.mode == "select-request-cancel":
                self._register_request()
            return
        if self.mode == "select-request-timeout":
            self._register_request()
            GLib.timeout_add(250, self._return_select_request, invocation)
            return
        if self.mode == "select-late-reply":
            self._register_request()
            GLib.timeout_add(70, self._return_select_request, invocation)
            return

        self._register_request()
        if self.mode == "select-transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "controlled SelectSources transport failure",
            )
            return
        invocation.return_value(GLib.Variant("(o)", (self.select_request_path,)))
        if self.mode.startswith(("start-", "streams-", "open-")):
            GLib.timeout_add(10, self._emit_response, 0, "empty")
        elif self.mode in {
            "select-success",
            "select-session-close-failure",
            "select-session-close-ambiguous",
        }:
            GLib.timeout_add(10, self._emit_response, 0, "empty")
        elif self.mode == "select-portal-cancel":
            GLib.timeout_add(10, self._emit_response, 1, "empty")
        elif self.mode == "select-portal-failure":
            GLib.timeout_add(10, self._emit_response, 2, "empty")
        elif self.mode == "select-malformed":
            GLib.timeout_add(10, self._emit_response, 9, "empty")
        elif self.mode == "select-malformed-signal":
            GLib.timeout_add(10, self._emit_malformed_signal)
        elif self.mode in {
            "select-response-timeout",
            "select-client-cancel",
            "select-request-close-failure",
            "select-request-session-cleanup-failure",
        }:
            pass

    def _start(self, sender, parameters, invocation):
        self.start_calls += 1
        session_handle, parent_window, options = parameters.unpack()
        request_token = self._string_option(options, "handle_token")
        self.start_options_valid &= (
            set(options.keys()) == {"handle_token"}
            and session_handle == self.session_path
            and parent_window == ""
            and request_token is not None
            and request_token != self.session_token
            and request_token != self.select_token
        )
        sender_component = self._sender_component(sender)
        if request_token is None:
            request_token = "invalid"
        self.start_request_path = (
            f"/org/freedesktop/portal/desktop/request/{sender_component}/{request_token}"
        )
        self.current_request_path = self.start_request_path
        if self.mode == "start-unanswered":
            return
        if self.mode == "start-request-timeout":
            self._register_request()
            GLib.timeout_add(250, self._return_start_request, invocation)
            return
        if self.mode == "start-late-reply":
            self._register_request()
            GLib.timeout_add(70, self._return_start_request, invocation)
            return

        self._register_request()
        if self.mode == "start-transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "controlled Start transport failure",
            )
            return
        invocation.return_value(GLib.Variant("(o)", (self.start_request_path,)))
        if self.mode.startswith(("streams-", "open-")):
            GLib.timeout_add(10, self._emit_response, 0, "streams")
        elif self.mode in {
            "start-success",
            "start-session-close-failure",
            "start-session-close-ambiguous",
        }:
            GLib.timeout_add(10, self._emit_response, 0, "streams")
        elif self.mode == "start-portal-cancel":
            GLib.timeout_add(10, self._emit_response, 1, "streams")
        elif self.mode == "start-portal-failure":
            GLib.timeout_add(10, self._emit_response, 2, "streams")
        elif self.mode == "start-malformed":
            GLib.timeout_add(10, self._emit_response, 9, "streams")
        elif self.mode == "start-malformed-signal":
            GLib.timeout_add(10, self._emit_malformed_signal)
        elif self.mode in {
            "start-response-timeout",
            "start-client-cancel",
            "start-request-close-failure",
            "start-request-session-cleanup-failure",
        }:
            pass

    def _return_start_request(self, invocation):
        if self.start_request_path is not None:
            invocation.return_value(GLib.Variant("(o)", (self.start_request_path,)))
        return GLib.SOURCE_REMOVE

    def _return_select_request(self, invocation):
        if self.select_request_path is not None:
            invocation.return_value(GLib.Variant("(o)", (self.select_request_path,)))
        return GLib.SOURCE_REMOVE

    def _open_pipe_wire_remote(self, parameters, invocation):
        self.open_calls += 1
        session_handle, options = parameters.unpack()
        self.open_options_valid &= (
            session_handle == self.session_path and not options
        )
        self._write_state()

        if self.mode in {
            "open-method-timeout",
            "open-response-timeout",
            "open-client-cancel",
            "open-ambiguous",
        }:
            return
        if self.mode == "open-transport-failure":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "controlled OpenPipeWireRemote transport failure",
            )
            return
        if self.mode == "open-unavailable":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.ServiceUnknown",
                "controlled unavailable PipeWire remote",
            )
            return
        if self.mode == "open-unsupported":
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.UnknownMethod",
                "controlled unsupported OpenPipeWireRemote",
            )
            return
        if self.mode == "open-wrong-reply":
            # Keep the outer D-Bus signature valid but point at a missing
            # UnixFDList entry.  This reaches the Rust deserializer as a
            # malformed direct-FD reply without relying on GDBus to emit an
            # invalid method signature (which it may leave unanswered).
            invocation.return_value_with_unix_fd_list(
                GLib.Variant("(h)", (42,)), Gio.UnixFDList.new()
            )
            return
        if self.mode == "open-malformed-reply":
            invocation.return_value_with_unix_fd_list(
                GLib.Variant("(h)", (0,)), Gio.UnixFDList.new()
            )
            return
        if self.mode == "open-late-transport":
            GLib.timeout_add(70, self._return_open_transport_error, invocation)
            return

        if self.mode == "open-late-fd":
            GLib.timeout_add(70, self._return_open_fd, invocation)
            return
        self._return_open_fd(invocation)

    @staticmethod
    def _new_test_fd():
        return os.open("/dev/null", os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))

    def _return_open_fd(self, invocation):
        fd = self._new_test_fd()
        self.open_fd_sent += 1
        self._write_state()
        # A D-Bus `h` is an index into the message's UnixFDList, not the
        # process-local descriptor number.  Keep the fake contract identical
        # to a real portal's direct-FD reply.
        fd_list = Gio.UnixFDList.new_from_array([fd])
        invocation.return_value_with_unix_fd_list(
            GLib.Variant("(h)", (0,)), fd_list
        )
        # GDBus queues the reply on the main loop. Keep the source descriptor
        # alive until that queued message has had a chance to attach the FD;
        # closing it in the same callback can turn a valid `h` reply into a
        # never-completing transport on the controlled bus.
        GLib.timeout_add(100, self._close_test_fd, fd)
        return GLib.SOURCE_REMOVE

    @staticmethod
    def _close_test_fd(fd):
        os.close(fd)
        return GLib.SOURCE_REMOVE

    @staticmethod
    def _return_open_transport_error(invocation):
        invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.Failed",
            "controlled late OpenPipeWireRemote transport failure",
        )
        return GLib.SOURCE_REMOVE

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
        if object_path == self.start_request_path:
            self.start_request_close_calls += 1
        elif object_path == self.select_request_path:
            self.select_request_close_calls += 1
        else:
            self.create_request_close_calls += 1
        if self.request_terminal.get(object_path, False):
            self.unexpected_close = True
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.portal.Error.UnexpectedCloseAfterResponse",
                "Request.Close is invalid after Response",
            )
            return
        if self.mode in {
            "select-request-close-failure",
            "select-request-session-cleanup-failure",
            "start-request-close-failure",
            "start-request-session-cleanup-failure",
        }:
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
        if object_path == self.select_request_path:
            self.select_request_active = False
        if object_path == self.start_request_path:
            self.start_request_active = False
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
        if self.fd_marker is not None:
            self.fd_close_before_session = os.path.exists(self.fd_marker)
        if self.mode in {
            "select-session-close-failure",
            "select-request-session-cleanup-failure",
            "start-session-close-failure",
            "start-request-session-cleanup-failure",
            "open-session-close-failure",
            "open-fd-session-cleanup-failure",
            "streams-valid-close-failure",
            "streams-malformed-close-failure",
        }:
            self._write_state()
            invocation.return_dbus_error(
                "org.freedesktop.DBus.Error.AccessDenied",
                "controlled session close failure",
            )
            return
        if self.mode in {"select-session-close-ambiguous", "start-session-close-ambiguous"}:
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

    def _emit_response(self, status, _kind):
        path = self.current_request_path
        if path not in self.request_registrations or self.request_terminal.get(path, False):
            return GLib.SOURCE_REMOVE
        self.request_terminal[path] = True
        self.request_active = False
        if path == self.select_request_path:
            self.select_request_active = False
        if path == self.start_request_path:
            self.start_request_active = False
        results = {}
        if path == self.create_request_path:
            results["session_handle"] = GLib.Variant("s", self.session_path)
        elif path == self.start_request_path:
            # This is deliberately realistic enough to exercise the boundary,
            # but the Rust Start adapter must drop it untouched.  No stream
            # fields are written to the state file.
            results["streams"] = GLib.Variant(
                "a(ua{sv})",
                [
                    (
                        42,
                        {
                            "source_type": GLib.Variant("u", 2),
                            "size": GLib.Variant("(ii)", (1280, 720)),
                            "id": GLib.Variant("s", "synthetic-window"),
                        },
                    )
                ],
            )
            self.start_streams_payload_emitted = True
            if self.mode.startswith("streams-"):
                results = self._stream_results()
        self._write_state()
        self.connection.emit_signal(
            None,
            path,
            REQUEST_INTERFACE,
            "Response",
            GLib.Variant("(ua{sv})", (status, results)),
        )
        return GLib.SOURCE_REMOVE

    def _stream_results(self):
        mode = self.mode
        properties = {"source_type": GLib.Variant("u", 2)}
        if mode in {"streams-missing", "streams-malformed-close-failure"}:
            return {}
        if mode == "streams-wrong-type":
            return {"streams": GLib.Variant("s", "private-stream-sentinel")}
        if mode == "streams-malformed-tuple":
            return {"streams": GLib.Variant("a(sa{sv})", [("private-node", {})])}
        if mode == "streams-malformed-container":
            return {"streams": GLib.Variant("a(uas)", [(424242, ["private-property"])])}
        if mode == "streams-extra":
            properties["future-property"] = GLib.Variant("s", "private-stream-sentinel")
        if mode == "streams-opaque":
            properties.update({"size": GLib.Variant("(ii)", (-1, -2147483648)),
                               "future-property": GLib.Variant("a{sv}", {"private": GLib.Variant("s", "private-stream-sentinel")})})
        if mode == "streams-minimal":
            properties = {}
        if mode == "streams-wrong-source":
            properties["source_type"] = GLib.Variant("u", 1)
        if mode == "streams-wrong-source-type":
            properties["source_type"] = GLib.Variant("s", "window")
        streams = [(424242, properties)]
        if mode == "streams-empty":
            streams = []
        if mode == "streams-multiple":
            streams *= 2
        return {"streams": GLib.Variant("a(ua{sv})", streams)}

    def _emit_malformed_signal(self):
        path = self.current_request_path
        if path not in self.request_registrations or self.request_terminal.get(path, False):
            return GLib.SOURCE_REMOVE
        self.request_terminal[path] = True
        self.request_active = False
        if path == self.select_request_path:
            self.select_request_active = False
        if path == self.start_request_path:
            self.start_request_active = False
        self._write_state()
        self.connection.emit_signal(
            None,
            path,
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
    parser.add_argument("--fd-marker")
    args = parser.parse_args()

    fake = FakePortal(args.mode, args.state_file, args.fd_marker)
    loop = GLib.MainLoop()

    def stop(_signum, _frame):
        loop.quit()

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    loop.run()
    fake.close()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fake ScreenCast SelectSources validation failed: {error}", file=sys.stderr)
        raise
