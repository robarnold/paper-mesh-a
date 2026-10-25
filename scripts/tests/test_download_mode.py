import array
from contextlib import ExitStack, contextmanager
import io
import json
import os
from pathlib import Path
import stat
import sys
import termios
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import enter_download_mode as helper

APP = ("cafe", "4010", "paper-mesh-prototype")
ROM = ("303a", "1001", "board-rom-serial")
PORT = Path("/dev/ttyACM0")
ACK = b'{"jsonrpc":"2.0","id":1,"result":{"accepted":true,"mode":"rom-download"}}\n'


@contextmanager
def fake_application(*_args):
    yield 55


class DownloadModeTests(unittest.TestCase):
    def test_exact_request_has_no_parameters_and_short_writes_send_only_the_remainder(self):
        accepted = bytearray()
        sizes = iter([3, 1, 5, 1000])

        def write(_fd, data):
            count = min(next(sizes), len(data))
            accepted.extend(data[:count])
            return count

        with patch.object(helper.select, "select", return_value=([], [55], [])), \
                patch.object(helper.os, "write", side_effect=write) as output:
            helper.send_request(55)
        self.assertEqual(bytes(accepted), helper.REQUEST)
        self.assertEqual(accepted.count(b"\n"), 1)
        self.assertEqual(json.loads(accepted), {"jsonrpc": "2.0", "id": 1, "method": "system.bootloader"})
        self.assertEqual(output.call_count, 4)

    def test_write_failure_does_not_restart_a_partially_sent_command(self):
        with patch.object(helper.select, "select", return_value=([], [55], [])), \
                patch.object(helper.os, "write", side_effect=[4, OSError("disconnected")]) as output:
            with self.assertRaises(helper.OutcomeUnknown):
                helper.send_request(55)
        self.assertEqual(output.call_args_list[1].args[1], helper.REQUEST[4:])
        self.assertEqual(output.call_count, 2)

    def test_zero_write_and_write_timeout_stop(self):
        with patch.object(helper.select, "select", return_value=([], [55], [])), \
                patch.object(helper.os, "write", return_value=0) as output:
            with self.assertRaises(helper.OutcomeUnknown):
                helper.send_request(55)
            output.assert_called_once()
        with patch.object(helper.time, "monotonic", side_effect=[0, 6]), \
                patch.object(helper.os, "write") as output:
            with self.assertRaises(helper.OutcomeUnknown):
                helper.send_request(55)
            output.assert_not_called()

    def test_acknowledgement_requires_exact_id_types_and_result(self):
        self.assertTrue(helper.parse_acknowledgement(ACK))
        for request_id in [2, "1", True, 1.0, None]:
            reply = json.loads(ACK)
            reply["id"] = request_id
            with self.subTest(request_id=request_id):
                self.assertFalse(helper.parse_acknowledgement(json.dumps(reply)))
        for result in [None, [], True, {"accepted": 1, "mode": "rom-download"},
                       {"accepted": False, "mode": "rom-download"},
                       {"accepted": True, "mode": "application"}, {"accepted": True}]:
            reply = {"jsonrpc": "2.0", "id": 1, "result": result}
            with self.subTest(result=result), self.assertRaises(helper.OutcomeUnknown):
                helper.parse_acknowledgement(json.dumps(reply))
        with self.assertRaises(helper.OutcomeUnknown):
            helper.parse_acknowledgement('{"jsonrpc":"1.0","id":1,"result":{}}')

    def test_firmware_error_stops_and_malformed_errors_are_ambiguous(self):
        response = {"jsonrpc": "2.0", "id": 1,
                    "error": {"code": -32001, "message": "Display busy"}}
        with self.assertRaisesRegex(helper.FirmwareRejected, "Display busy"):
            helper.parse_acknowledgement(json.dumps(response))
        for error in [None, "busy", {"code": True, "message": "busy"},
                      {"code": -1, "message": 5}]:
            response["error"] = error
            with self.subTest(error=error), self.assertRaises(helper.OutcomeUnknown):
                helper.parse_acknowledgement(json.dumps(response))
        response.update(error={"code": -1, "message": "busy"}, result={})
        with self.assertRaises(helper.OutcomeUnknown):
            helper.parse_acknowledgement(json.dumps(response))

    def test_fragmented_acknowledgement_ignores_logs_and_other_requests(self):
        with patch.object(helper.select, "select", return_value=([55], [], [])), \
                patch.object(helper.os, "read", side_effect=[
                    b"INFO display: idle\nnot JSON\n{\"id\":2}\n" + ACK[:12], ACK[12:],
                ]) as read:
            helper.wait_for_acknowledgement(55)
        self.assertEqual(read.call_count, 2)

    def test_disconnection_timeout_and_unbounded_lines_are_ambiguous(self):
        for chunks in [[b""], [OSError("disconnected")], [b"x" * 4096] * 3]:
            with self.subTest(chunks=chunks), \
                    patch.object(helper.select, "select", return_value=([55], [], [])), \
                    patch.object(helper.os, "read", side_effect=chunks), \
                    self.assertRaises(helper.OutcomeUnknown):
                helper.wait_for_acknowledgement(55)
        with patch.object(helper.time, "monotonic", side_effect=[0, 6]), \
                self.assertRaises(helper.OutcomeUnknown):
            helper.wait_for_acknowledgement(55)

    def test_exact_rom_identity_already_present_sends_nothing(self):
        with patch.object(helper, "find_port", return_value=PORT) as find, \
                patch.object(helper, "identity", return_value=ROM), \
                patch.object(helper, "open_application") as opened, \
                patch.object(helper, "send_request") as send:
            result = helper.enter_download_mode(ROM[2])
        find.assert_called_once_with(ROM)
        opened.assert_not_called()
        send.assert_not_called()
        self.assertEqual(result["status"], "already-in-rom-download")
        self.assertFalse(result["command_sent"])

    def test_wrong_identity_or_arbitrary_port_never_opens(self):
        for port, found_identity in [(PORT, ("cafe", "4010", "other-board")),
                                     (Path("/dev/ttyS0"), APP),
                                     (Path("/tmp/ttyACM0"), APP)]:
            with self.subTest(port=port, identity=found_identity), \
                    patch.object(helper, "find_port", side_effect=[None, port]), \
                    patch.object(helper, "identity", return_value=found_identity), \
                    patch.object(helper.os, "open") as opened, self.assertRaises(ValueError):
                helper.enter_download_mode(ROM[2])
            opened.assert_not_called()

    def test_missing_application_sends_nothing(self):
        with patch.object(helper, "find_port", return_value=None), \
                patch.object(helper, "send_request") as send, self.assertRaises(ValueError):
            helper.enter_download_mode(ROM[2])
        send.assert_not_called()

    def test_open_is_exclusive_nonblocking_raw_without_flushing_and_asserts_only_dtr(self):
        device = SimpleNamespace(st_mode=stat.S_IFCHR, st_rdev=os.makedev(166, 0))
        with ExitStack() as stack:
            stack.enter_context(patch.object(helper, "identity", return_value=APP))
            opened = stack.enter_context(patch.object(helper.os, "open", return_value=55))
            stack.enter_context(patch.object(helper.os, "fstat", return_value=device))
            stack.enter_context(patch.object(helper.os, "stat", return_value=device))
            ioctl = stack.enter_context(patch.object(helper.fcntl, "ioctl"))
            raw = stack.enter_context(patch.object(helper.tty, "setraw"))
            closed = stack.enter_context(patch.object(helper.os, "close"))
            with helper.open_application(PORT, APP) as fd:
                self.assertEqual(fd, 55)
            opened.assert_called_once_with(PORT, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
            raw.assert_called_once_with(55, when=termios.TCSANOW)
            self.assertEqual([call.args[1] for call in ioctl.call_args_list],
                             [termios.TIOCEXCL, termios.TIOCMBIS, termios.TIOCNXCL])
            self.assertEqual(ioctl.call_args_list[1].args[2], array.array("i", [termios.TIOCM_DTR]))
            closed.assert_called_once_with(55)

    def test_identity_change_after_open_prevents_tty_changes_and_command(self):
        device = SimpleNamespace(st_mode=stat.S_IFCHR, st_rdev=os.makedev(166, 0))
        with patch.object(helper, "identity", side_effect=[APP, ROM]), \
                patch.object(helper.os, "open", return_value=55), \
                patch.object(helper.os, "fstat", return_value=device), \
                patch.object(helper.os, "stat", return_value=device), \
                patch.object(helper.os, "close") as close, \
                patch.object(helper.fcntl, "ioctl") as ioctl:
            with self.assertRaises(ValueError), helper.open_application(PORT, APP):
                self.fail("The changed identity must not enter the context.")
        ioctl.assert_not_called()
        close.assert_called_once_with(55)

    def test_acknowledged_command_still_requires_the_expected_rom(self):
        with patch.object(helper, "matching_port", side_effect=[None, PORT]), \
                patch.object(helper, "open_application", side_effect=fake_application), \
                patch.object(helper, "send_request") as send, \
                patch.object(helper, "wait_for_acknowledgement") as ack, \
                patch.object(helper.time, "monotonic", return_value=100), \
                patch.object(helper, "wait_for_application_detach") as detach, \
                patch.object(helper, "wait_for_rom", return_value=Path("/dev/ttyACM1")) as rom:
            result = helper.enter_download_mode(ROM[2])
        send.assert_called_once_with(55)
        ack.assert_called_once_with(55)
        detach.assert_called_once_with(APP, deadline=110)
        rom.assert_called_once_with(ROM, deadline=110)
        self.assertEqual(result["status"], "rom-download")
        self.assertTrue(result["command_sent"])
        self.assertTrue(result["acknowledged"])

    def test_application_detaches_then_closes_before_rom_wait_with_the_ack_deadline(self):
        events = []
        clock = [100]

        @contextmanager
        def application(*_args):
            events.append("open")
            try:
                yield 55
            finally:
                events.append("close")
                clock[0] += 5

        def wait_detach(expected, *, deadline):
            self.assertEqual(events, ["open", "request", "ack"])
            self.assertEqual(expected, APP)
            self.assertEqual(deadline, 110)
            events.append("detach")
            clock[0] += 2

        def wait_rom(expected, *, deadline):
            self.assertEqual(events, ["open", "request", "ack", "detach", "close"])
            self.assertEqual(expected, ROM)
            self.assertEqual(clock[0], 107)
            self.assertEqual(deadline, 110)
            events.append("rom")
            return PORT

        with patch.object(helper, "matching_port", side_effect=[None, PORT]), \
                patch.object(helper, "open_application", side_effect=application), \
                patch.object(helper, "send_request", side_effect=lambda _fd: events.append("request")), \
                patch.object(helper, "wait_for_acknowledgement", side_effect=lambda _fd: events.append("ack")), \
                patch.object(helper.time, "monotonic", side_effect=lambda: clock[0]), \
                patch.object(helper, "wait_for_application_detach", side_effect=wait_detach), \
                patch.object(helper, "wait_for_rom", side_effect=wait_rom) as rom:
            result = helper.enter_download_mode(ROM[2])
        rom.assert_called_once()
        self.assertEqual(events[-1], "rom")
        self.assertEqual(result["status"], "rom-download")

    def test_detach_wait_keeps_polling_until_the_exact_application_is_absent(self):
        with patch.object(helper.time, "monotonic", return_value=100), \
                patch.object(helper.time, "sleep") as sleep, \
                patch.object(helper, "find_port", side_effect=[PORT, PORT, None]) as find:
            helper.wait_for_application_detach(APP, deadline=110)
        self.assertEqual(find.call_count, 3)
        self.assertTrue(all(call.args == (APP,) for call in find.call_args_list))
        self.assertEqual(sleep.call_count, 2)
        self.assertTrue(all(call.args == (0.01,) for call in sleep.call_args_list))

    def test_application_that_remains_connected_reaches_the_shared_deadline(self):
        with patch.object(helper.time, "monotonic", side_effect=range(100, 120)), \
                patch.object(helper.time, "sleep"), \
                patch.object(helper, "find_port", return_value=PORT), \
                self.assertRaisesRegex(helper.OutcomeUnknown, "did not detach"):
            helper.wait_for_application_detach(APP, deadline=110)

    def test_unconfirmed_detachment_does_not_retry_or_start_rom_verification(self):
        with patch.object(helper, "matching_port", side_effect=[None, PORT]), \
                patch.object(helper, "open_application", side_effect=fake_application), \
                patch.object(helper, "send_request") as send, \
                patch.object(helper, "wait_for_acknowledgement") as ack, \
                patch.object(helper.time, "monotonic", return_value=100), \
                patch.object(helper, "wait_for_application_detach", side_effect=helper.OutcomeUnknown("did not detach")), \
                patch.object(helper, "wait_for_rom") as rom, \
                self.assertRaises(helper.OutcomeUnknown):
            helper.enter_download_mode(ROM[2])
        send.assert_called_once_with(55)
        ack.assert_called_once_with(55)
        rom.assert_not_called()

    def test_expired_ack_deadline_does_not_start_another_rom_wait(self):
        with patch.object(helper.time, "monotonic", return_value=111), \
                patch.object(helper, "matching_port") as find, \
                self.assertRaises(helper.OutcomeUnknown):
            helper.wait_for_rom(ROM, deadline=110)
        find.assert_not_called()

    def test_rejection_or_ambiguous_reply_never_repeats_or_waits_for_rom(self):
        for error in [helper.FirmwareRejected("busy"), helper.OutcomeUnknown("no acknowledgement")]:
            with self.subTest(error=error), \
                    patch.object(helper, "matching_port", side_effect=[None, PORT]), \
                    patch.object(helper, "open_application", side_effect=fake_application), \
                    patch.object(helper, "send_request") as send, \
                    patch.object(helper, "wait_for_acknowledgement", side_effect=error), \
                    patch.object(helper, "wait_for_rom") as rom, \
                    self.assertRaises(type(error)):
                helper.enter_download_mode(ROM[2])
            send.assert_called_once_with(55)
            rom.assert_not_called()

    def test_rom_failure_after_acknowledgement_never_repeats_the_command(self):
        with patch.object(helper, "matching_port", side_effect=[None, PORT]), \
                patch.object(helper, "open_application", side_effect=fake_application), \
                patch.object(helper, "send_request") as send, \
                patch.object(helper, "wait_for_acknowledgement") as ack, \
                patch.object(helper.time, "monotonic", return_value=100), \
                patch.object(helper, "wait_for_application_detach"), \
                patch.object(helper, "wait_for_rom", side_effect=helper.OutcomeUnknown("missing ROM")) as rom, \
                self.assertRaises(helper.OutcomeUnknown):
            helper.enter_download_mode(ROM[2])
        send.assert_called_once_with(55)
        ack.assert_called_once_with(55)
        rom.assert_called_once_with(ROM, deadline=110)

    def test_missing_or_wrong_rom_times_out_without_another_command(self):
        with patch.object(helper.time, "monotonic", side_effect=range(30)), \
                patch.object(helper.time, "sleep"), \
                patch.object(helper, "find_port", return_value=None) as find, \
                patch.object(helper, "send_request") as send, \
                self.assertRaisesRegex(helper.OutcomeUnknown, "specified ROM USB identity did not appear"):
            helper.wait_for_rom(ROM)
        self.assertGreater(find.call_count, 0)
        self.assertTrue(all(call.args == (ROM,) for call in find.call_args_list))
        send.assert_not_called()
        with patch.object(helper, "find_port", return_value=PORT), \
                patch.object(helper, "identity", return_value=("303a", "1001", "wrong-serial")), \
                self.assertRaises(helper.OutcomeUnknown):
            helper.wait_for_rom(ROM)

    def test_cli_requires_rom_serial_and_container_and_has_no_port_override(self):
        for argv in [[], ["--rom-serial", "serial", "--port", "/dev/ttyACM0"]]:
            with self.subTest(argv=argv), patch("sys.stderr", io.StringIO()), \
                    self.assertRaises(SystemExit) as exit_status:
                helper.main(argv)
            self.assertEqual(exit_status.exception.code, 2)
        with patch.dict(os.environ, {"PAPER_MESH_DEVCONTAINER": "0"}), \
                patch("sys.stderr", io.StringIO()), self.assertRaises(SystemExit):
            helper.main(["--rom-serial", "serial"])

    def test_cli_reports_uncertainty_without_claiming_rom_success(self):
        lock = Mock()
        lock.__enter__ = Mock(return_value=lock)
        lock.__exit__ = Mock(return_value=False)
        with patch.dict(os.environ, {"PAPER_MESH_DEVCONTAINER": "1"}), \
                patch("builtins.open", return_value=lock), \
                patch.object(helper.fcntl, "flock"), \
                patch.object(helper, "enter_download_mode", side_effect=helper.OutcomeUnknown("missing ROM")), \
                patch("sys.stderr", io.StringIO()) as error, patch("sys.stdout", io.StringIO()) as output:
            code = helper.main(["--rom-serial", "serial"])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(error.getvalue())["status"], "outcome-unknown")
        self.assertEqual(output.getvalue(), "")

    def test_cli_interruption_reports_uncertainty(self):
        with patch.dict(os.environ, {"PAPER_MESH_DEVCONTAINER": "1"}), \
                patch.object(helper.fcntl, "flock", side_effect=KeyboardInterrupt), \
                patch("sys.stderr", io.StringIO()) as error, patch("sys.stdout", io.StringIO()) as output:
            code = helper.main(["--rom-serial", "serial"])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(error.getvalue())["status"], "outcome-unknown")
        self.assertEqual(output.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
