#!/usr/bin/env python3
"""Request ROM download mode once and verify the specified USB identity."""

import argparse
import array
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import re
import select
import stat
import sys
import termios
import time
import tty

from device_usb import find_port, identity

REQUEST = b'{"jsonrpc":"2.0","id":1,"method":"system.bootloader"}\n'
APPLICATION_SERIAL = "paper-mesh-prototype"
ACK_TIMEOUT = 5.0
ROM_TIMEOUT = 10.0
MAX_LINE_BYTES = 8192


class OutcomeUnknown(RuntimeError):
    """The request might have taken effect, but the required evidence is absent."""


class FirmwareRejected(RuntimeError):
    """The matching JSON-RPC response rejected the request."""


def check_port(port, expected):
    port = Path(port)
    if port.parent != Path("/dev") or not re.fullmatch(r"ttyACM\d+", port.name):
        raise ValueError("Only an identified USB CDC device can receive this request.")
    if identity(Path("/sys/class/tty") / port.name) != expected:
        raise ValueError("The USB device identity changed. No request was sent.")
    return port


def matching_port(expected):
    port = find_port(expected)
    return check_port(port, expected) if port is not None else None


@contextmanager
def open_application(port, expected):
    port = check_port(port, expected)
    fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    exclusive = False
    try:
        opened = os.fstat(fd)
        if not stat.S_ISCHR(opened.st_mode) or opened.st_rdev != os.stat(port).st_rdev:
            raise ValueError("The opened device does not match the identified USB port.")
        check_port(port, expected)
        fcntl.ioctl(fd, termios.TIOCEXCL)
        exclusive = True
        tty.setraw(fd, when=termios.TCSANOW)
        fcntl.ioctl(fd, termios.TIOCMBIS, array.array("i", [termios.TIOCM_DTR]))
        check_port(port, expected)
        yield fd
    finally:
        if exclusive:
            try:
                fcntl.ioctl(fd, termios.TIOCNXCL)
            except OSError:
                pass
        try:
            os.close(fd)
        except OSError:
            pass


def send_request(fd, timeout=ACK_TIMEOUT):
    """Complete one command, including short writes, without sending it again."""
    deadline = time.monotonic() + timeout
    sent = 0
    while sent < len(REQUEST):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise OutcomeUnknown("Command write timed out. The request will not be repeated.")
        try:
            if not select.select([], [fd], [], min(0.1, remaining))[1]:
                continue
            count = os.write(fd, REQUEST[sent:])
        except (BlockingIOError, InterruptedError):
            continue
        except OSError as error:
            raise OutcomeUnknown("USB failed during the command write. No retry was attempted.") from error
        if not 0 < count <= len(REQUEST) - sent:
            raise OutcomeUnknown("USB did not complete the command write. No retry was attempted.")
        sent += count


def parse_acknowledgement(line):
    """Ignore logs and other request IDs. Accept only the specified response."""
    try:
        response = json.loads(line)
    except (ValueError, UnicodeError):
        return False
    if not isinstance(response, dict):
        return False
    request_id = response.get("id")
    if type(request_id) is not int or request_id != 1:
        return False
    if response.get("jsonrpc") != "2.0":
        raise OutcomeUnknown("The matching response has an invalid JSON-RPC version.")
    if "error" in response:
        error = response["error"]
        if ("result" in response or not isinstance(error, dict)
                or type(error.get("code")) is not int
                or not isinstance(error.get("message"), str)):
            raise OutcomeUnknown("The matching response has an invalid error object.")
        raise FirmwareRejected(f"Firmware rejected the request ({error['code']}): {error['message']}")
    result = response.get("result")
    if (not isinstance(result, dict) or result.get("accepted") is not True
            or result.get("mode") != "rom-download"):
        raise OutcomeUnknown("The matching response did not confirm ROM download mode.")
    return True


def wait_for_acknowledgement(fd, timeout=ACK_TIMEOUT):
    deadline = time.monotonic() + timeout
    pending = b""
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise OutcomeUnknown("No matching acknowledgement arrived. The request will not be repeated.")
        try:
            if not select.select([fd], [], [], min(0.1, remaining))[0]:
                continue
            chunk = os.read(fd, 4096)
        except (BlockingIOError, InterruptedError):
            continue
        except OSError as error:
            raise OutcomeUnknown("USB disconnected before acknowledgement. No retry was attempted.") from error
        if not chunk:
            raise OutcomeUnknown("USB closed before acknowledgement. No retry was attempted.")
        pending += chunk
        while b"\n" in pending:
            raw, pending = pending.split(b"\n", 1)
            if len(raw) > MAX_LINE_BYTES:
                raise OutcomeUnknown("The USB response exceeded the line limit.")
            if parse_acknowledgement(raw.rstrip(b"\r")):
                return
        if len(pending) > MAX_LINE_BYTES:
            raise OutcomeUnknown("The USB response exceeded the line limit.")


def wait_for_application_detach(expected, *, deadline):
    """Keep DTR asserted until firmware disconnects its application interface."""
    while time.monotonic() < deadline:
        try:
            if find_port(expected) is None:
                return
        except (OSError, ValueError) as error:
            raise OutcomeUnknown("The request was accepted, but application detachment could not be verified.") from error
        time.sleep(min(0.01, max(0, deadline - time.monotonic())))
    raise OutcomeUnknown("The request was accepted, but application USB did not detach. No retry was attempted.")


def wait_for_rom(expected, timeout=ROM_TIMEOUT, *, deadline=None):
    if deadline is None:
        deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            port = matching_port(expected)
        except (OSError, ValueError) as error:
            raise OutcomeUnknown("The request was accepted, but ROM identity verification failed.") from error
        if port is not None:
            return port
        time.sleep(min(0.1, max(0, deadline - time.monotonic())))
    raise OutcomeUnknown("The request was accepted, but the specified ROM USB identity did not appear.")


def enter_download_mode(rom_serial, application_serial=APPLICATION_SERIAL):
    if not isinstance(rom_serial, str) or not rom_serial.strip():
        raise ValueError("An exact ROM serial is required.")
    if not isinstance(application_serial, str) or not application_serial.strip():
        raise ValueError("An exact application serial is required.")
    rom_identity = ("303a", "1001", rom_serial)
    port = matching_port(rom_identity)
    if port is not None:
        return {"status": "already-in-rom-download", "rom_serial": rom_serial,
                "rom_port": str(port), "command_sent": False, "acknowledged": False}
    application_identity = ("cafe", "4010", application_serial)
    port = matching_port(application_identity)
    if port is None:
        raise ValueError("The specified application USB identity is absent. No request was sent.")
    with open_application(port, application_identity) as fd:
        send_request(fd)
        wait_for_acknowledgement(fd)
        rom_deadline = time.monotonic() + ROM_TIMEOUT
        # Firmware checks DTR once more after its reply. Let firmware initiate detachment.
        wait_for_application_detach(application_identity, deadline=rom_deadline)
    # Release the old ACM device during the firmware's detach interval when possible.
    rom_port = wait_for_rom(rom_identity, deadline=rom_deadline)
    return {"status": "rom-download", "rom_serial": rom_serial,
            "rom_port": str(rom_port), "command_sent": True, "acknowledged": True}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom-serial", required=True, help="Exact serial of the expected ROM USB device.")
    parser.add_argument("--application-serial", default=APPLICATION_SERIAL)
    args = parser.parse_args(argv)
    if os.environ.get("PAPER_MESH_DEVCONTAINER") != "1":
        parser.error("Run this script in the USB devcontainer.")
    try:
        with open("/tmp/paper-mesh-cdc.lock", "w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = enter_download_mode(args.rom_serial, args.application_serial)
    except FirmwareRejected as error:
        print(json.dumps({"status": "firmware-rejected", "error": str(error), "retried": False}), file=sys.stderr)
        return 1
    except OutcomeUnknown as error:
        print(json.dumps({"status": "outcome-unknown", "error": str(error),
                          "retried": False, "reset_attempted": False}), file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print(json.dumps({"status": "outcome-unknown",
                          "error": "The helper was interrupted. The device state was not confirmed.",
                          "retried": False, "reset_attempted": False}), file=sys.stderr)
        return 2
    except (OSError, ValueError) as error:
        print(json.dumps({"status": "request-not-sent", "error": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
