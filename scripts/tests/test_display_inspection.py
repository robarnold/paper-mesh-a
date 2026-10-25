from contextlib import contextmanager
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import zlib

sys.path.insert(0, str(Path(__file__).parents[1]))
import inspect_display as helper


NATIVE = (bytes(range(256)) * ((helper.FRAME_BYTES + 255) // 256))[:helper.FRAME_BYTES]
CHECKSUM = helper.checksum(NATIVE)


def inspection():
    return {"protocol": 1, "stage": "black", "frame_id": 23, "captured_at_ms": 100,
            "recovery_blocked": False,
            "framebuffer": {"width": 960, "height": 540, "format": "gray4",
                            "packing": "low-nibble-first", "length": helper.FRAME_BYTES,
                            "checksum": CHECKSUM, "committed_checksum": CHECKSUM,
                            "shutdown_complete": True,
                            "most_recent_refresh": {"success": True, "shutdown_confirmed": True,
                                                    "frame_checksum": CHECKSUM}}}


class FakeRpc:
    def __init__(self, after=None, change=None):
        self.calls = []
        self.inspections = 0
        self.after = after or inspection()
        self.change = change

    def call(self, method, params=None):
        self.calls.append((method, params))
        if method == "display.inspect":
            self.inspections += 1
            return inspection() if self.inspections == 1 else self.after
        if method != "display.framebuffer.read":
            raise AssertionError("Unexpected device method")
        offset, length = params["offset"], params["length"]
        result = {**params, "hex": NATIVE[offset:offset + length].hex()}
        if self.change:
            self.change(result)
        return result


def response(result, request_id=1):
    return (json.dumps({"jsonrpc": "2.0", "id": request_id, "result": result}) + "\n").encode()


class DisplayInspectionTests(unittest.TestCase):
    def test_frame_download_has_exact_chunks_and_same_final_identity(self):
        rpc = FakeRpc()
        saved = []
        result = helper.capture_channel(rpc, lambda name, value: saved.append((name, value)))
        self.assertEqual(result["native"], NATIVE)
        self.assertEqual([name for name, _ in saved], ["before", "after"])
        chunks = [params for method, params in rpc.calls if method == "display.framebuffer.read"]
        self.assertEqual(len(chunks), 507)
        self.assertEqual(chunks[0], {"frame_id": 23, "offset": 0, "length": 512})
        self.assertEqual(chunks[-1], {"frame_id": 23, "offset": 259072, "length": 128})
        self.assertEqual(sum(chunk["length"] for chunk in chunks), helper.FRAME_BYTES)
        self.assertEqual({method for method, _ in rpc.calls}, {"display.inspect", "display.framebuffer.read"})

    def test_invalid_snapshot_stops_before_chunk_requests(self):
        cases = []
        for field, value in (("protocol", 2), ("protocol", True), ("frame_id", True), ("frame_id", 0),
                             ("frame_id", -1), ("captured_at_ms", 1 << 64),
                             ("stage", ""), ("recovery_blocked", True)):
            item = inspection()
            item[field] = value
            cases.append(item)
        for field, value in (("width", 540), ("height", 960), ("length", 1), ("format", "rgb8"),
                             ("packing", "high-nibble-first"), ("checksum", True),
                             ("committed_checksum", None), ("committed_checksum", CHECKSUM ^ 1),
                             ("shutdown_complete", False), ("most_recent_refresh", None)):
            item = inspection()
            item["framebuffer"][field] = value
            cases.append(item)
        for field in ("success", "shutdown_confirmed", "frame_checksum"):
            item = inspection()
            item["framebuffer"]["most_recent_refresh"][field] = None
            cases.append(item)
        for value in cases:
            rpc = Mock()
            rpc.call.return_value = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                helper.capture_channel(rpc)
            rpc.call.assert_called_once_with("display.inspect")

    def test_chunk_identity_bounds_and_hex_are_strict(self):
        good = {"frame_id": 23, "offset": 512, "length": 2, "hex": "00fF"}
        self.assertEqual(helper.validate_chunk(good, 23, 512, 2), b"\x00\xff")
        for field, value in (("frame_id", 24), ("frame_id", True), ("offset", 0), ("offset", 512.0),
                             ("length", 3), ("hex", "00 ff"), ("hex", "00gg"), ("hex", "00"), ("hex", None)):
            item = {**good, field: value}
            with self.subTest(item=item), self.assertRaises(ValueError):
                helper.validate_chunk(item, 23, 512, 2)

    def test_changed_chunk_stops_without_a_second_chunk(self):
        rpc = FakeRpc(change=lambda result: result.update(frame_id=24))
        with self.assertRaisesRegex(ValueError, "frame_id"):
            helper.capture_channel(rpc)
        self.assertEqual(len(rpc.calls), 2)

    def test_corrupt_content_fails_checksum_before_final_snapshot(self):
        rpc = FakeRpc(change=lambda result: result.update(hex="00" * result["length"]))
        with self.assertRaisesRegex(ValueError, "checksum"):
            helper.capture_channel(rpc)
        self.assertEqual(rpc.inspections, 1)

    def test_changed_final_frame_stage_checksum_and_shutdown_are_rejected(self):
        for field, value in (("frame_id", 24), ("stage", "partial-white"), ("captured_at_ms", 99),
                             ("recovery_blocked", True), ("checksum", CHECKSUM ^ 1),
                             ("shutdown_complete", False)):
            after = inspection()
            if field in after:
                after[field] = value
            else:
                after["framebuffer"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                helper.capture_channel(FakeRpc(after=after))

    def test_fnv1a_reference_vectors(self):
        self.assertEqual(helper.checksum(b""), 0x811C9DC5)
        self.assertEqual(helper.checksum(b"a"), 0xE40C292C)
        self.assertEqual(helper.checksum(b"foobar"), 0xBF9CF968)

    def test_rotation_uses_low_nibble_first_and_opposite_native_rows(self):
        native = bytearray(helper.FRAME_BYTES)
        native[0] = 0x21
        native[479] = 0x43
        native[539 * 480] = 0x65
        native[-1] = 0x87
        pixels = helper.portrait_pixels(native)
        expected = {(539, 0): 1, (539, 1): 2, (539, 958): 3, (539, 959): 4,
                    (0, 0): 5, (0, 1): 6, (0, 958): 7, (0, 959): 8}
        for (x, y), gray in expected.items():
            self.assertEqual(pixels[y * 540 + x], gray * 17)
        self.assertEqual(sum(value != 0 for value in pixels), 8)

    def test_png_crc_dimensions_and_scanlines(self):
        pixels = helper.portrait_pixels(NATIVE)
        encoded = helper.png(pixels)
        self.assertEqual(encoded[:8], b"\x89PNG\r\n\x1a\n")
        offset = 8
        kinds = []
        while offset < len(encoded):
            length = struct.unpack_from(">I", encoded, offset)[0]
            kind = encoded[offset + 4:offset + 8]
            data = encoded[offset + 8:offset + 8 + length]
            crc = struct.unpack_from(">I", encoded, offset + 8 + length)[0]
            self.assertEqual(crc, zlib.crc32(kind + data) & 0xFFFFFFFF)
            kinds.append(kind)
            if kind == b"IHDR":
                self.assertEqual(struct.unpack(">IIBBBBB", data), (540, 960, 8, 0, 0, 0, 0))
            elif kind == b"IDAT":
                decoded = zlib.decompress(data)
                self.assertEqual(len(decoded), 541 * 960)
                self.assertTrue(all(decoded[y * 541] == 0 for y in range(960)))
                self.assertEqual(b"".join(decoded[y * 541 + 1:(y + 1) * 541] for y in range(960)), pixels)
            offset += length + 12
        self.assertEqual(kinds, [b"IHDR", b"IDAT", b"IEND"])

    def test_histogram_covers_the_whole_frame_and_all_gray_levels(self):
        pixels = bytearray([255]) * (540 * 960)
        for level in range(16):
            pixels[level] = level * 17
        pixels[-1] = 0
        result = helper.histograms(pixels)
        self.assertEqual(result["counts"], [2] + [1] * 14 + [540 * 960 - 16])
        self.assertEqual(sum(result["counts"]), 540 * 960)
        self.assertEqual((result["width"], result["height"]), (540, 960))
        self.assertNotIn("bands", result)
        for invalid in [pixels[:-1], bytes([1]) + pixels[1:]]:
            with self.assertRaises(ValueError):
                helper.histograms(invalid)

    def test_rpc_handles_fragmented_input_short_writes_and_omits_payload_logs(self):
        wire = b"plain log\r\n" + b'{"jsonrpc":"2.0","method":"log","params":"hidden"}\n' + response({"hex": "abcdef"})
        accepted = bytearray()
        events = []

        def write(_fd, data):
            count = min(7, len(data))
            accepted.extend(data[:count])
            return count

        with patch.object(helper.select, "select", return_value=([55], [55], [])), \
                patch.object(helper.os, "read", side_effect=[wire[i:i + 3] for i in range(0, len(wire), 3)]), \
                patch.object(helper.os, "write", side_effect=write):
            rpc = helper.Rpc(55, events.append)
            self.assertEqual(rpc.call("display.inspect"), {"hex": "abcdef"})
        self.assertEqual(json.loads(accepted), {"jsonrpc": "2.0", "id": 1, "method": "display.inspect"})
        self.assertNotIn("abcdef", json.dumps(events))
        self.assertNotIn("hidden", json.dumps(events))
        self.assertEqual([event["event"] for event in events], ["request", "log", "notification", "response"])

    def test_only_read_methods_and_valid_outbound_chunks_can_reach_usb(self):
        cases = [("system.bootloader", None), ("display.test.start", None), ("display.drive.read", None), ("display.inspect", {}),
                 ("display.framebuffer.read", {"frame_id": 0, "offset": 0, "length": 1}),
                 ("display.framebuffer.read", {"frame_id": 1, "offset": 0, "length": 0}),
                 ("display.framebuffer.read", {"frame_id": 1, "offset": 0, "length": 513}),
                 ("display.framebuffer.read", {"frame_id": 1, "offset": helper.FRAME_BYTES - 1, "length": 2}),
                 ("display.framebuffer.read", {"frame_id": True, "offset": 0, "length": 1})]
        for method, params in cases:
            with self.subTest(method=method, params=params), patch.object(helper.os, "write") as write, self.assertRaises(ValueError):
                helper.Rpc(55).call(method, params)
            write.assert_not_called()

    def test_rpc_rejects_bad_envelopes_errors_and_ambiguous_json(self):
        lines = [response({}, request_id=2), response({}, request_id=True),
                 b'{"jsonrpc":"1.0","id":1,"result":{}}\n',
                 b'{"jsonrpc":"2.0","id":1,"result":{},"error":{}}\n',
                 b'{"jsonrpc":"2.0","id":1,"result":NaN}\n',
                 b'{"jsonrpc":"2.0","id":1,"id":1,"result":{}}\n',
                 b'{"jsonrpc":"2.0","id":1,"result":null}\n',
                 b'{"jsonrpc":"2.0","id":1,"error":{"code":-1,"message":"busy"}}\n',
                 b'{broken\n']
        for wire in lines:
            with self.subTest(wire=wire), patch.object(helper.select, "select", return_value=([55], [55], [])), \
                    patch.object(helper.os, "read", return_value=wire), \
                    patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)), self.assertRaises(ValueError):
                helper.Rpc(55).call("display.inspect")

    def test_rpc_line_byte_time_and_disconnect_bounds(self):
        for wire in (b"x" * 4096, b"x" * 4095 + b"\n", b""):
            with self.subTest(wire_length=len(wire)), patch.object(helper.select, "select", return_value=([55], [55], [])), \
                    patch.object(helper.os, "read", return_value=wire), \
                    patch.object(helper.os, "write", side_effect=lambda _fd, data: len(data)), self.assertRaises(ValueError):
                helper.Rpc(55).call("display.inspect")
        with patch.object(helper.time, "monotonic", side_effect=[0, 0, 6]), \
                patch.object(helper.os, "write") as write, self.assertRaises(TimeoutError):
            helper.Rpc(55).call("display.inspect")
        write.assert_not_called()

    def test_changed_capture_never_publishes_binary_or_preview(self):
        @contextmanager
        def opened(_port, _identity):
            yield 55

        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "capture"
            with patch.object(helper.fcntl, "flock"), \
                    patch.object(helper, "matching_port", return_value=Path("/dev/ttyACM7")), \
                    patch.object(helper, "open_application", side_effect=opened), \
                    patch.object(helper, "Rpc", return_value=FakeRpc(change=lambda result: result.update(frame_id=99))), \
                    self.assertRaises(ValueError):
                helper.capture(output)
            self.assertTrue((output / "before.json").exists())
            for name in ("native.bin", "portrait.png", "summary.json"):
                self.assertFalse((output / name).exists())

    def test_capture_uses_one_exact_identity_connection_and_saves_all_artifacts(self):
        @contextmanager
        def opened(_port, _identity):
            yield 55

        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "capture"
            with patch.object(helper.fcntl, "flock"), \
                    patch.object(helper, "matching_port", return_value=Path("/dev/ttyACM7")) as match, \
                    patch.object(helper, "open_application", side_effect=opened) as connection, \
                    patch.object(helper, "check_port") as verify, \
                    patch.object(helper, "Rpc", return_value=FakeRpc()):
                summary = helper.capture(output)
            match.assert_called_once_with(("cafe", "4010", "paper-mesh-prototype"))
            connection.assert_called_once_with(Path("/dev/ttyACM7"), helper.APPLICATION_IDENTITY)
            verify.assert_called_once_with(Path("/dev/ttyACM7"), helper.APPLICATION_IDENTITY)
            self.assertEqual((output / "native.bin").read_bytes(), NATIVE)
            self.assertEqual(set(path.name for path in output.iterdir()),
                             {"before.json", "after.json", "native.bin", "portrait.png",
                              "histogram.json", "events.jsonl", "summary.json"})
            self.assertEqual(summary["checksum"], CHECKSUM)
            self.assertFalse(summary["optical_state_measured"])
            self.assertEqual(json.loads((output / "summary.json").read_text()), summary)

    def test_absent_identity_never_opens_usb(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(helper.fcntl, "flock"), \
                patch.object(helper, "matching_port", return_value=None), \
                patch.object(helper, "open_application") as connection, self.assertRaises(ValueError):
            helper.capture(Path(temporary) / "capture")
        connection.assert_not_called()


if __name__ == "__main__":
    unittest.main()
