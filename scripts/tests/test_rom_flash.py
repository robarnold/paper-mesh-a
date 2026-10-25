from contextlib import ExitStack
import os
from pathlib import Path
import stat
import struct
import sys
import termios
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import prepare_rom_flash as helper


def response(opcode, value=0, data=b"", status=b"\0\0\0\0"):
    packet = struct.pack("<BBHI", 1, opcode, len(data) + len(status), value) + data + status
    return b"\xc0" + packet.replace(b"\xdb", b"\xdb\xdd").replace(b"\xc0", b"\xdb\xdc") + b"\xc0"


class FakeRom:
    def __init__(self, strap=8, option=1, chip=9, secure=False):
        self.strap = strap
        self.option = option
        self.security = struct.pack("<I", 4 if secure else 0) + bytes(8) + struct.pack("<II", chip, 0)
        self.commands = []
        self.registers = {}

    def command(self, opcode, payload=b"", timeout=helper.COMMAND_TIMEOUT):
        self.commands.append((opcode, payload))
        if opcode == helper.GET_SECURITY_INFO:
            return 0, self.security
        if opcode == helper.WRITE_REG:
            address, value, mask, delay = struct.unpack("<IIII", payload)
            if delay != 0:
                raise AssertionError("Unexpected register write")
            if address == helper.OPTION1:
                self.option = (self.option & ~mask) | (value & mask)
            else:
                self.registers[address] = (self.registers.get(address, 0) & ~mask) | (value & mask)
        return 0, b""

    def read_register(self, address):
        if address == helper.GPIO_STRAP:
            return self.strap
        if address == helper.OPTION1:
            return self.option
        return self.registers.get(address, 0)

    def writes(self):
        return [struct.unpack("<IIII", payload) for opcode, payload in self.commands if opcode == helper.WRITE_REG]


class RomFlashTests(unittest.TestCase):
    def test_request_header_and_slip_escaping(self):
        self.assertEqual(helper.frame(helper.READ_REG, b"\xc0\xdb"),
                         b"\xc0\x00\x0a\x02\x00\x00\x00\x00\x00\xdb\xdc\xdb\xdd\xc0")

    def test_fragmented_escaped_response_and_remaining_sync_packets(self):
        wire = b"boot log\r\n" + response(helper.SYNC) * 7 + response(helper.READ_REG, 0xDBC0)
        accepted = bytearray()

        def write(_fd, data):
            count = min(3, len(data))
            accepted.extend(data[:count])
            return count

        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "read", side_effect=[wire[index:index + 3] for index in range(0, len(wire), 3)]), \
                patch.object(helper.os, "write", side_effect=write):
            self.assertEqual(helper.Rom(55).read_register(helper.OPTION1), 0xDBC0)
        self.assertEqual(bytes(accepted), helper.frame(helper.READ_REG, struct.pack("<I", helper.OPTION1)))

    def test_malformed_response_and_error_stop(self):
        bad = [
            b"\xc0\xdb\x01\xc0",
            b"\xc0\xdb\xc0",
            b"\xc0" + b"x" * 65,
            b"\xc0" + struct.pack("<BBHI", 1, helper.READ_REG, 8, 0) + bytes(4) + b"\xc0",
            response(helper.READ_REG, status=b"\0\0"),
            response(helper.READ_REG, status=b"\x01\x05\0\0"),
            response(helper.WRITE_REG),
        ]
        for wire in bad:
            with self.subTest(wire=wire), \
                    patch.object(helper.select, "select", return_value=([55], [55], [])), \
                    patch.object(helper.os, "read", return_value=wire), \
                    patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)), \
                    self.assertRaises(ValueError):
                helper.Rom(55).read_register(helper.OPTION1)

    def test_noise_and_unrelated_sync_packets_are_bounded(self):
        for wire in (b"x" * 256, response(helper.SYNC) * 10):
            with self.subTest(wire=wire), \
                    patch.object(helper.select, "select", return_value=([55], [55], [])), \
                    patch.object(helper.os, "read", return_value=wire), \
                    patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)), \
                    self.assertRaises(ValueError):
                helper.Rom(55).read_register(helper.OPTION1)

    def test_timeout_zero_write_and_disconnect_stop_without_retry(self):
        with patch.object(helper.time, "monotonic", side_effect=[0, 4]), \
                patch.object(helper.os, "write") as write, self.assertRaises(ValueError):
            helper.Rom(55).command(helper.SYNC)
        write.assert_not_called()
        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "write", return_value=0) as write, self.assertRaises(ValueError):
            helper.Rom(55).command(helper.SYNC)
        write.assert_called_once()
        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)) as write, \
                patch.object(helper.os, "read", return_value=b""), self.assertRaises(ValueError):
            helper.Rom(55).command(helper.SYNC)
        write.assert_called_once()

    def test_flash_reset_and_other_commands_are_not_available(self):
        with patch.object(helper.os, "write") as write, self.assertRaises(ValueError):
            helper.Rom(55).command(0x04)
        write.assert_not_called()

    def test_wrong_chip_and_secure_mode_never_write_registers(self):
        for rom in (FakeRom(chip=5), FakeRom(secure=True)):
            for operation in (helper.prepare_channel,
                              lambda rom, verify: helper.finalize_channel(rom, verify, 1)):
                with self.subTest(rom=rom, operation=operation), self.assertRaises(ValueError):
                    operation(rom, Mock())
                self.assertEqual(rom.writes(), [])

    def test_prepare_preserves_force_bit_and_selects_exit(self):
        for option in (0, 0xA4, 0xA5):
            for strap in (0, 8):
                verify = Mock()
                rom = FakeRom(strap=strap, option=option)
                result = helper.prepare_channel(rom, verify)
                expected = "no-reset" if option & 1 else ("hard-reset" if strap & 8 else "watchdog-reset")
                self.assertEqual(result, {"after": expected, "strap": strap,
                                          "finalize_required": bool(option & 1),
                                          "option1_before": option, "option1_after": option})
                verify.assert_called_once()
                self.assertEqual(rom.writes(), [])

    def test_finalization_clears_only_force_bit_then_requests_rtc_reset(self):
        rom = FakeRom(option=0xA5)
        verify = Mock()
        result = helper.finalize_channel(rom, verify, 0xA5)
        self.assertEqual(result["option1_before"], 0xA5)
        self.assertEqual(result["option1_after"], 0xA4)
        self.assertEqual(result["status"], "reset-requested")
        self.assertTrue(result["reset_requested"])
        self.assertTrue(result["reset_acknowledged"])
        self.assertEqual(result["sync_attempts"], 1)
        self.assertEqual(verify.call_count, 2)
        self.assertEqual(rom.writes(), [
            (helper.OPTION1, 0, 1, 0),
            (helper.RTC_WDT_PROTECT, 0x50D83AA1, 0xFFFFFFFF, 0),
            (helper.RTC_WDT_CONFIG1, 2000, 0xFFFFFFFF, 0),
            (helper.RTC_WDT_CONFIG0, 0xD0000102, 0xFFFFFFFF, 0),
            (helper.RTC_WDT_PROTECT, 0, 0xFFFFFFFF, 0),
        ])

    def test_finalization_rejects_missing_or_changed_request_without_writes(self):
        for expected, current in ((0, 1), (True, 1), (-1, 1), (0x100000001, 1), (1, 0), (1, 3)):
            rom = FakeRom(option=current)
            with self.subTest(expected=expected, current=current), self.assertRaises(ValueError):
                helper.finalize_channel(rom, Mock(), expected)
            self.assertEqual(rom.writes(), [])

    def test_failed_readback_or_changed_identity_stops(self):
        for after in (0xA5, 0):
            rom = FakeRom(option=0xA5)
            rom.read_register = Mock(side_effect=[0xA5, after])
            with self.subTest(after=after), self.assertRaises(ValueError):
                helper.finalize_channel(rom, Mock(), 0xA5)
            self.assertEqual(rom.writes(), [(helper.OPTION1, 0, 1, 0)])
        rom = FakeRom()
        with self.assertRaises(ValueError):
            helper.finalize_channel(rom, Mock(side_effect=ValueError("USB identity changed")), 1)
        self.assertEqual(rom.writes(), [])

    def test_timeout_readback_failure_does_not_arm_reset(self):
        rom = FakeRom()
        rom.read_register = Mock(side_effect=[1, 0, 0])
        with self.assertRaisesRegex(ValueError, "timeout did not match"):
            helper.finalize_channel(rom, Mock(), 1)
        self.assertNotIn(helper.RTC_WDT_CONFIG0, [item[0] for item in rom.writes()])

    def test_lost_final_ack_reports_uncertain_reset_without_retry(self):
        for fail_on_arm in (True, False):
            rom = FakeRom()
            original = rom.command

            def command(opcode, payload=b"", timeout=helper.COMMAND_TIMEOUT):
                result = original(opcode, payload, timeout=timeout)
                if opcode == helper.WRITE_REG:
                    address, value, _, _ = struct.unpack("<IIII", payload)
                    if (fail_on_arm and address == helper.RTC_WDT_CONFIG0) or (
                            not fail_on_arm and address == helper.RTC_WDT_PROTECT and value == 0):
                        raise OSError("USB disconnected")
                return result

            rom.command = command
            with self.subTest(fail_on_arm=fail_on_arm), self.assertRaises(helper.ResetRequestError) as raised:
                helper.finalize_channel(rom, Mock(), 1)
            self.assertTrue(raised.exception.reset_attempted)
            self.assertEqual(raised.exception.reset_acknowledged, not fail_on_arm)
            self.assertEqual(len([item for item in rom.writes() if item[0] == helper.RTC_WDT_CONFIG0]), 1)
            self.assertEqual(len(rom.writes()), 4 if fail_on_arm else 5)

    def test_finalizer_waits_for_exact_rom_then_attempts_once(self):
        port = Path("/dev/ttyACM4")
        rom = FakeRom()
        with patch.object(helper, "matching_ports", side_effect=[[], [port]]) as ports, \
                patch.object(helper.time, "sleep") as sleep, \
                patch.object(helper, "open_rom") as opened, \
                patch.object(helper, "Rom", return_value=rom), \
                patch.object(helper, "check_port"):
            opened.return_value.__enter__.return_value = 55
            result = helper.finalize("board", 1)
        self.assertEqual(result["status"], "reset-requested")
        self.assertEqual(ports.call_count, 2)
        sleep.assert_called_once_with(0.1)
        opened.assert_called_once_with(port, ("303a", "1001", "board"))

    def test_finalizer_missing_or_duplicate_port_does_not_open(self):
        for ports in ([], [Path("/dev/ttyACM1"), Path("/dev/ttyACM2")]):
            with self.subTest(ports=ports), \
                    patch.object(helper, "matching_ports", return_value=ports), \
                    patch.object(helper.time, "monotonic", side_effect=[0, 11]), \
                    patch.object(helper, "open_rom") as opened, self.assertRaises(ValueError):
                helper.finalize("board", 1)
            opened.assert_not_called()

    def test_finalizer_rejects_stub_before_register_writes(self):
        wire = response(helper.SYNC, status=b"\0\0")
        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "read", return_value=wire), \
                patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)) as writes, \
                self.assertRaisesRegex(ValueError, "not an ESP32-S3 ROM"):
            helper.finalize_channel(helper.Rom(55), Mock(), 1)
        self.assertEqual(writes.call_count, 1)

    def test_finalizer_retries_only_initial_sync_timeout_then_writes_once(self):
        rom = FakeRom()
        original = rom.command

        def command(opcode, payload=b"", timeout=helper.COMMAND_TIMEOUT):
            result = original(opcode, payload, timeout=timeout)
            if len(rom.commands) == 1:
                raise helper.RomTimeout("Reset still in progress", helper.SYNC)
            return result

        rom.command = command
        with patch.object(helper.time, "sleep"):
            result = helper.finalize_channel(rom, Mock(), 1)
        self.assertEqual(result["sync_attempts"], 2)
        self.assertEqual([opcode for opcode, _ in rom.commands[:3]],
                         [helper.SYNC, helper.SYNC, helper.GET_SECURITY_INFO])
        self.assertEqual(len(rom.writes()), 5)
        self.assertEqual(len([item for item in rom.writes() if item[0] == helper.OPTION1]), 1)

    def test_sync_readiness_retry_has_deadline_and_attempt_limit(self):
        rom = FakeRom()
        rom.command = Mock(side_effect=helper.RomTimeout("Not ready", helper.SYNC))
        with patch.object(helper.time, "monotonic", return_value=0), \
                patch.object(helper.time, "sleep"), self.assertRaises(helper.RomTimeout) as raised:
            helper.finalize_channel(rom, Mock(), 1)
        self.assertEqual(rom.command.call_count, helper.SYNC_ATTEMPTS)
        self.assertEqual(raised.exception.sync_attempts, helper.SYNC_ATTEMPTS)
        with patch.object(helper.time, "monotonic", return_value=11), \
                self.assertRaises(helper.RomTimeout):
            helper.finalize_channel(rom, Mock(), 1, readiness_deadline=10)
        self.assertEqual(rom.command.call_count, helper.SYNC_ATTEMPTS)

    def test_protocol_errors_wrong_chip_and_later_timeouts_do_not_retry(self):
        for error in (ValueError("Invalid response"), helper.RomTimeout("Other command", helper.READ_REG)):
            rom = FakeRom()
            rom.command = Mock(side_effect=error)
            with self.subTest(error=error), self.assertRaises(ValueError):
                helper.finalize_channel(rom, Mock(), 1)
            self.assertEqual(rom.command.call_count, 1)
        rom = FakeRom(chip=5)
        with self.assertRaises(ValueError):
            helper.finalize_channel(rom, Mock(), 1)
        self.assertEqual([opcode for opcode, _ in rom.commands], [helper.SYNC, helper.GET_SECURITY_INFO])
        self.assertEqual(rom.writes(), [])
        rom = FakeRom()
        original = rom.command

        def command(opcode, payload=b"", timeout=helper.COMMAND_TIMEOUT):
            result = original(opcode, payload, timeout=timeout)
            if opcode == helper.GET_SECURITY_INFO:
                raise helper.RomTimeout("Not ready", opcode)
            return result

        rom.command = command
        with self.assertRaises(helper.RomTimeout):
            helper.finalize_channel(rom, Mock(), 1)
        self.assertEqual([opcode for opcode, _ in rom.commands], [helper.SYNC, helper.GET_SECURITY_INFO])
        self.assertEqual(rom.writes(), [])

    def test_delayed_sync_responses_from_retries_remain_bounded(self):
        wire = response(helper.SYNC) * 31 + response(helper.READ_REG, 1)
        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "read", return_value=wire), \
                patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)):
            self.assertEqual(helper.Rom(55).read_register(helper.OPTION1), 1)

    def test_rom_open_uses_raw_mode_without_control_line_changes(self):
        expected = ("303a", "1001", "test-board")
        port = Path("/dev/ttyACM2")
        metadata = SimpleNamespace(st_mode=stat.S_IFCHR, st_rdev=42)
        settings = [0, 0, termios.HUPCL, 0, 0, 0, []]
        with ExitStack() as stack:
            stack.enter_context(patch.object(helper, "identity", return_value=expected))
            opened = stack.enter_context(patch.object(helper.os, "open", return_value=55))
            stack.enter_context(patch.object(helper.os, "fstat", return_value=metadata))
            stack.enter_context(patch.object(helper.os, "stat", return_value=metadata))
            closed = stack.enter_context(patch.object(helper.os, "close"))
            ioctl = stack.enter_context(patch.object(helper.fcntl, "ioctl"))
            raw = stack.enter_context(patch.object(helper.tty, "setraw"))
            stack.enter_context(patch.object(helper.termios, "tcgetattr", return_value=settings))
            configured = stack.enter_context(patch.object(helper.termios, "tcsetattr"))
            with helper.open_rom(port, expected) as fd:
                self.assertEqual(fd, 55)
        opened.assert_called_once_with(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        raw.assert_called_once_with(55, when=termios.TCSANOW)
        self.assertEqual([call.args[1] for call in ioctl.call_args_list], [termios.TIOCEXCL, termios.TIOCNXCL])
        self.assertEqual(configured.call_args.args[1], termios.TCSANOW)
        self.assertEqual(configured.call_args.args[2][2] & termios.HUPCL, 0)
        self.assertEqual(configured.call_args.args[2][4:6], [termios.B115200, termios.B115200])
        closed.assert_called_once_with(55)

    def test_wrong_usb_identity_and_missing_serial_never_open(self):
        with patch.object(helper, "identity", return_value=("303a", "1001", "other")), \
                patch.object(helper.os, "open") as opened, self.assertRaises(ValueError):
            helper.prepare(Path("/dev/ttyACM0"), "expected")
        opened.assert_not_called()
        with patch.object(helper.os, "open") as opened, self.assertRaises(ValueError):
            helper.prepare(Path("/dev/ttyACM0"), "")
        opened.assert_not_called()


if __name__ == "__main__":
    unittest.main()
