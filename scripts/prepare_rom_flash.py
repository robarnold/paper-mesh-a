#!/usr/bin/env python3
"""Check one ROM downloader and finalize a completed software-entry flash."""

import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import re
import select
import stat
import struct
import sys
import termios
import time
import tty

# ESP32-S3 protocol and esptool 5.1.0 register definitions:
# https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/serial-protocol.html
# https://github.com/espressif/esptool/blob/v5.1.0/esptool/targets/esp32s3.py#L328-L351
SYNC = 0x08
WRITE_REG = 0x09
READ_REG = 0x0A
GET_SECURITY_INFO = 0x14
OPTION1 = 0x6000812C
GPIO_STRAP = 0x60004038
RTC_WDT_CONFIG0 = 0x60008098
RTC_WDT_CONFIG1 = 0x6000809C
RTC_WDT_PROTECT = 0x600080B0
RTC_WDT_KEY = 0x50D83AA1
RTC_WDT_RESET = 0xD0000102
RTC_WDT_TIMEOUT = 2000
PORT_TIMEOUT = 10.0
MAX_FRAME = 64
MAX_RESPONSE_BYTES = 4096
COMMAND_TIMEOUT = 3.0
SYNC_ATTEMPTS = 4


class RomTimeout(ValueError):
    """A bounded ROM command did not receive a response."""

    def __init__(self, message, opcode=None):
        self.opcode = opcode
        super().__init__(message)


def identity(port):
    for parent in (Path("/sys/class/tty") / Path(port).name / "device").resolve().parents:
        try:
            return tuple((parent / name).read_text().strip()
                         for name in ("idVendor", "idProduct", "serial"))
        except OSError:
            continue
    return None


def check_port(port, expected):
    port = Path(port)
    if port.parent != Path("/dev") or not re.fullmatch(r"ttyACM\d+", port.name):
        raise ValueError("Only an identified USB CDC port can receive ROM commands.")
    if identity(port) != expected:
        raise ValueError("The ROM USB identity does not match the specified board.")
    return port


@contextmanager
def open_rom(port, expected):
    port = check_port(port, expected)
    fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    exclusive = False
    try:
        opened = os.fstat(fd)
        if not stat.S_ISCHR(opened.st_mode) or opened.st_rdev != os.stat(port).st_rdev:
            raise ValueError("The opened device does not match the identified ROM port.")
        check_port(port, expected)
        fcntl.ioctl(fd, termios.TIOCEXCL)
        exclusive = True
        tty.setraw(fd, when=termios.TCSANOW)
        settings = termios.tcgetattr(fd)
        settings[2] = (settings[2] | termios.CLOCAL | termios.CREAD) & ~termios.HUPCL
        settings[4] = settings[5] = termios.B115200
        termios.tcsetattr(fd, termios.TCSANOW, settings)
        check_port(port, expected)
        yield fd
    finally:
        if exclusive:
            try:
                fcntl.ioctl(fd, termios.TIOCNXCL)
            except OSError:
                pass
        os.close(fd)


def frame(opcode, payload):
    packet = struct.pack("<BBHI", 0, opcode, len(payload), 0) + payload
    return b"\xc0" + packet.replace(b"\xdb", b"\xdb\xdd").replace(b"\xc0", b"\xdb\xdc") + b"\xc0"


class Rom:
    """Use bounded ROM commands for flash checks and the final reset request."""

    def __init__(self, fd):
        self.fd = fd
        self.pending = bytearray()

    def _ready(self, deadline, writing=False):
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RomTimeout("The ROM command timed out.")
            try:
                readers, writers, _ = select.select(
                    [] if writing else [self.fd], [self.fd] if writing else [], [], min(remaining, 0.1))
                if (writers if writing else readers):
                    return
            except InterruptedError:
                continue

    def _write(self, data, deadline):
        sent = 0
        while sent < len(data):
            self._ready(deadline, writing=True)
            try:
                count = os.write(self.fd, data[sent:])
            except (BlockingIOError, InterruptedError):
                continue
            if not 0 < count <= len(data) - sent:
                raise ValueError("USB did not complete the ROM command write.")
            sent += count

    def _read_frame(self, deadline, budget):
        started = False
        escaped = False
        result = bytearray()
        while True:
            if not self.pending:
                self._ready(deadline)
                try:
                    chunk = os.read(self.fd, 256)
                except (BlockingIOError, InterruptedError):
                    continue
                if not chunk:
                    raise ValueError("USB closed during the ROM command.")
                self.pending.extend(chunk)
            value = self.pending.pop(0)
            budget[0] -= 1
            if budget[0] < 0:
                raise ValueError("The ROM response exceeded the byte limit.")
            if value == 0xC0:
                if escaped:
                    raise ValueError("The ROM response has an incomplete SLIP escape.")
                if result:
                    return bytes(result)
                started = True
                continue
            if not started:
                continue
            if escaped:
                if value not in (0xDC, 0xDD):
                    raise ValueError("The ROM response has an invalid SLIP escape.")
                result.append(0xC0 if value == 0xDC else 0xDB)
                escaped = False
            elif value == 0xDB:
                escaped = True
            else:
                result.append(value)
            if len(result) > MAX_FRAME:
                raise ValueError("The ROM response exceeded the frame limit.")

    def command(self, opcode, payload=b"", timeout=COMMAND_TIMEOUT):
        try:
            return self._command(opcode, payload, timeout)
        except RomTimeout as error:
            raise RomTimeout(f"ROM command 0x{opcode:02x} timed out.", opcode) from error

    def _command(self, opcode, payload, timeout):
        if opcode not in (SYNC, READ_REG, WRITE_REG, GET_SECURITY_INFO):
            raise ValueError("This helper cannot send that ROM command.")
        deadline = time.monotonic() + timeout
        self._write(frame(opcode, payload), deadline)
        budget = [MAX_RESPONSE_BYTES]
        # Four readiness attempts can leave up to 31 delayed SYNC responses.
        for _ in range(SYNC_ATTEMPTS * 8 + 1):
            packet = self._read_frame(deadline, budget)
            if len(packet) < 12:
                raise ValueError("The response is not an ESP32-S3 ROM response.")
            direction, response_opcode, size, value = struct.unpack("<BBHI", packet[:8])
            if direction != 1 or size != len(packet) - 8 or size < 4:
                raise ValueError("The ROM response has an invalid header.")
            if packet[-4] != 0 or packet[-3] != 0:
                raise ValueError(f"The ROM rejected command 0x{response_opcode:02x} (error 0x{packet[-3]:02x}).")
            if response_opcode == opcode:
                return value, packet[8:-4]
            # A single SYNC produces eight responses. Consume its remaining replies.
            if response_opcode != SYNC or size != 4:
                raise ValueError("The ROM response does not match the pending command.")
        raise ValueError("The ROM sent too many unrelated responses.")

    def read_register(self, address):
        value, data = self.command(READ_REG, struct.pack("<I", address))
        if data:
            raise ValueError("The ROM register response has unexpected data.")
        return value


def identify_channel(rom, readiness_deadline=None):
    attempts = 0
    while True:
        timeout = COMMAND_TIMEOUT
        if readiness_deadline is not None:
            timeout = min(timeout, readiness_deadline - time.monotonic())
            if timeout <= 0:
                raise RomTimeout("ROM synchronization did not finish before the readiness deadline.", SYNC)
        attempts += 1
        try:
            _, sync_data = rom.command(SYNC, b"\x07\x07\x12\x20" + b"\x55" * 32, timeout=timeout)
            break
        except RomTimeout as error:
            error.phase = "rom-sync"
            error.sync_attempts = attempts
            if (error.opcode != SYNC or readiness_deadline is None or attempts >= SYNC_ATTEMPTS
                    or time.monotonic() >= readiness_deadline):
                raise
            # espflash sends the stub reset without waiting for ROM readiness.
            # Retry only SYNC. No register write has occurred at this point.
            time.sleep(min(0.1, max(0, readiness_deadline - time.monotonic())))
    if sync_data:
        raise ValueError("The ROM sync response has unexpected data.")
    _, security = rom.command(GET_SECURITY_INFO)
    if len(security) != 20 or struct.unpack_from("<I", security, 12)[0] != 9:
        raise ValueError("The ROM did not identify an ESP32-S3. No register was written.")
    if struct.unpack_from("<I", security)[0] & (1 << 2):
        raise ValueError("Secure download mode does not permit this helper. No register was written.")
    return attempts


def prepare_channel(rom, verify_identity):
    identify_channel(rom)
    strap = rom.read_register(GPIO_STRAP)
    before = rom.read_register(OPTION1)
    verify_identity()
    # Keep FORCE_DOWNLOAD_BOOT set throughout flashing. Espressif clears it at exit.
    # With this bit set, espflash's no-reset path returns the stub to ROM.
    forced = bool(before & 1)
    # espflash 4.6.0 only performs its S3 watchdog reset when GPIO_STRAP.bit3 is clear.
    after = "no-reset" if forced else ("hard-reset" if strap & 8 else "watchdog-reset")
    return {"after": after, "finalize_required": forced,
            "strap": strap, "option1_before": before, "option1_after": before}


class ResetRequestError(ValueError):
    """The final reset write might have reached the board. Do not retry it."""

    reset_attempted = True

    def __init__(self, error, acknowledged):
        self.reset_acknowledged = acknowledged
        super().__init__(f"The reset request may have reached the board: {error}. "
                         "Do not retry. Check USB and application startup without another reset.")


def write_register(rom, address, value, mask=0xFFFFFFFF):
    _, data = rom.command(WRITE_REG, struct.pack("<IIII", address, value, mask, 0))
    if data:
        raise ValueError("The ROM write response has unexpected data.")


def check_forced_option(expected_option1):
    if type(expected_option1) is not int or not 0 <= expected_option1 <= 0xFFFFFFFF or not expected_option1 & 1:
        raise ValueError("Finalization requires the saved software download request with bit 0 set.")


def finalize_channel(rom, verify_identity, expected_option1, *, readiness_deadline=None):
    """Clear the retained request only after a successful flash, then request reset."""
    check_forced_option(expected_option1)
    if readiness_deadline is None:
        readiness_deadline = time.monotonic() + PORT_TIMEOUT
    sync_attempts = identify_channel(rom, readiness_deadline)
    before = rom.read_register(OPTION1)
    if before != expected_option1:
        raise ValueError("The software download request changed after the flash. No register was written.")
    verify_identity()
    write_register(rom, OPTION1, 0, 1)
    after = rom.read_register(OPTION1)
    if after != before & ~1:
        raise ValueError("The software download request did not clear with the other bits preserved.")
    verify_identity()
    # Match esptool 5.1.0: clear FORCE_DOWNLOAD_BOOT immediately before reset.
    # Its RTC reset includes the chip reset, which samples the boot straps again.
    # https://github.com/espressif/esptool/blob/v5.1.0/esptool/targets/esp32s3.py#L319-L351
    write_register(rom, RTC_WDT_PROTECT, RTC_WDT_KEY)
    write_register(rom, RTC_WDT_CONFIG1, RTC_WDT_TIMEOUT)
    if rom.read_register(RTC_WDT_CONFIG1) != RTC_WDT_TIMEOUT:
        raise ValueError("The RTC reset timeout did not match. No reset was requested.")
    acknowledged = False
    try:
        write_register(rom, RTC_WDT_CONFIG0, RTC_WDT_RESET)
        acknowledged = True
        write_register(rom, RTC_WDT_PROTECT, 0)
    except (OSError, ValueError) as error:
        # A reset can remove USB before either final response reaches the host.
        # Repetition could reset an application that has already started.
        raise ResetRequestError(error, acknowledged) from error
    return {"status": "reset-requested", "reset_requested": True,
            "reset_attempted": True, "reset_acknowledged": True,
            "sync_attempts": sync_attempts,
            "option1_before": before, "option1_after": after,
            "reset_method": "rtc-watchdog", "wdt_config0": RTC_WDT_RESET,
            "wdt_timeout": RTC_WDT_TIMEOUT}


def expected_identity(expected_rom_serial):
    if not isinstance(expected_rom_serial, str) or not expected_rom_serial.strip():
        raise ValueError("An exact ROM serial is required.")
    return "303a", "1001", expected_rom_serial


def prepare(port, expected_rom_serial):
    expected = expected_identity(expected_rom_serial)
    with open_rom(port, expected) as fd:
        return prepare_channel(Rom(fd), lambda: check_port(port, expected))


def matching_ports(expected):
    return [Path("/dev") / entry.name for entry in Path("/sys/class/tty").glob("ttyACM*")
            if identity(entry) == expected]


def finalize(expected_rom_serial, expected_option1):
    """Reacquire ROM after a successful flash. Never retry a finalization attempt."""
    expected = expected_identity(expected_rom_serial)
    check_forced_option(expected_option1)
    deadline = time.monotonic() + PORT_TIMEOUT
    while True:
        ports = matching_ports(expected)
        if len(ports) > 1:
            raise ValueError("More than one matching ROM USB device was found.")
        if ports:
            port = ports[0]
            with open_rom(port, expected) as fd:
                return finalize_channel(Rom(fd), lambda: check_port(port, expected), expected_option1,
                                        readiness_deadline=deadline)
        if time.monotonic() >= deadline:
            raise ValueError("The matching ROM USB device did not return. No reset was requested.")
        time.sleep(0.1)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom-serial", required=True)
    args = parser.parse_args(argv)
    if os.environ.get("PAPER_MESH_DEVCONTAINER") != "1":
        parser.error("Run this script in the USB devcontainer.")
    try:
        expected = ("303a", "1001", args.rom_serial)
        ports = matching_ports(expected)
        if len(ports) != 1:
            raise ValueError("Exactly one matching ROM USB device is required.")
        with open("/tmp/paper-mesh-cdc.lock", "w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = prepare(ports[0], args.rom_serial)
    except (OSError, ValueError) as error:
        print(json.dumps({"status": "not-prepared", "error": str(error), "reset_attempted": False}), file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
