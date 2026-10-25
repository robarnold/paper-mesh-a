#!/usr/bin/env python3
"""Capture the committed display framebuffer without a refresh or reset."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import re
import select
import struct
import sys
import time
import zlib

from enter_download_mode import check_port, matching_port, open_application

APPLICATION_IDENTITY = ("cafe", "4010", "paper-mesh-prototype")
FRAME_BYTES = 259_200
CHUNK_BYTES = 512
MAX_LINE_BYTES = 8192
MAX_RESPONSE_BYTES = 65_536
COMMAND_TIMEOUT = 5.0
CAPTURE_TIMEOUT = 180.0
U64_MAX = (1 << 64) - 1


def uint(value, maximum, name):
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError(f"Invalid {name}.")
    return value


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("The JSON response contains a duplicate field.")
        result[key] = value
    return result


def reject_constant(_value):
    raise ValueError("Invalid JSON number.")


class Rpc:
    """Send only the two display inspection methods over one USB connection."""

    def __init__(self, fd, event=None):
        self.fd = fd
        self.pending = bytearray()
        self.request_id = 0
        self.event = event or (lambda _record: None)
        self.deadline = time.monotonic() + CAPTURE_TIMEOUT

    def ready(self, deadline, writing=False):
        while True:
            remaining = min(deadline, self.deadline) - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("The display inspection timed out.")
            try:
                readers, writers, _ = select.select(
                    [] if writing else [self.fd], [self.fd] if writing else [], [], min(remaining, 0.1))
                if (writers if writing else readers):
                    return
            except InterruptedError:
                continue

    def line(self, deadline, budget):
        while True:
            if time.monotonic() >= min(deadline, self.deadline):
                raise TimeoutError("The display inspection timed out.")
            newline = self.pending.find(b"\n")
            if newline >= 0:
                if newline > MAX_LINE_BYTES:
                    raise ValueError("The USB response exceeded the line limit.")
                result = bytes(self.pending[:newline]).rstrip(b"\r")
                del self.pending[:newline + 1]
                return result
            if len(self.pending) > MAX_LINE_BYTES:
                raise ValueError("The USB response exceeded the line limit.")
            self.ready(deadline)
            try:
                chunk = os.read(self.fd, 4096)
            except (BlockingIOError, InterruptedError):
                continue
            if not chunk:
                raise ValueError("USB closed during display inspection.")
            budget[0] -= len(chunk)
            if budget[0] < 0:
                raise ValueError("The USB response exceeded the byte limit.")
            self.pending.extend(chunk)

    def validate_request(self, method, params):
        if method == "display.inspect":
            if params is not None:
                raise ValueError("Display inspection does not accept parameters.")
        elif method == "display.framebuffer.read":
            if not isinstance(params, dict) or set(params) != {"frame_id", "offset", "length"}:
                raise ValueError("Invalid framebuffer parameters.")
            uint(params["frame_id"], U64_MAX, "frame ID")
            uint(params["offset"], FRAME_BYTES - 1, "chunk offset")
            uint(params["length"], CHUNK_BYTES, "chunk length")
            if params["frame_id"] == 0 or params["length"] == 0 or params["offset"] + params["length"] > FRAME_BYTES:
                raise ValueError("The requested chunk is outside the framebuffer.")
        else:
            raise ValueError("This helper cannot send that method.")

    def call(self, method, params=None):
        self.validate_request(method, params)
        self.request_id += 1
        request = {"jsonrpc": "2.0", "id": self.request_id, "method": method}
        if params is not None:
            request["params"] = params
        wire = (json.dumps(request, separators=(",", ":")) + "\n").encode()
        deadline = min(self.deadline, time.monotonic() + COMMAND_TIMEOUT)
        sent = 0
        while sent < len(wire):
            self.ready(deadline, writing=True)
            try:
                count = os.write(self.fd, wire[sent:])
            except (BlockingIOError, InterruptedError):
                continue
            if not 0 < count <= len(wire) - sent:
                raise ValueError("USB did not complete the inspection request.")
            sent += count
        self.event({"event": "request", "id": self.request_id, "method": method, "params": params})
        budget = [MAX_RESPONSE_BYTES]
        while True:
            raw = self.line(deadline, budget)
            if not raw.lstrip().startswith(b"{"):
                self.event({"event": "log", "bytes": len(raw)})
                continue
            try:
                response = json.loads(raw, object_pairs_hook=unique_object, parse_constant=reject_constant)
            except (ValueError, UnicodeError) as error:
                raise ValueError("The USB response contains invalid JSON.") from error
            if not isinstance(response, dict):
                raise ValueError("The USB response is not an object.")
            if "id" not in response:
                self.event({"event": "notification", "bytes": len(raw)})
                continue
            if type(response["id"]) is not int or response["id"] != self.request_id:
                raise ValueError("The USB response has a different request ID.")
            if response.get("jsonrpc") != "2.0" or ("result" in response) == ("error" in response):
                raise ValueError("The USB response has an invalid JSON-RPC envelope.")
            if "error" in response:
                error = response["error"]
                if not isinstance(error, dict) or type(error.get("code")) is not int or not isinstance(error.get("message"), str):
                    raise ValueError("The USB response has an invalid error object.")
                self.event({"event": "rejected", "id": self.request_id, "code": error["code"]})
                raise ValueError(f"Firmware rejected {method} with code {error['code']}.")
            if not isinstance(response["result"], dict):
                raise ValueError("The USB response result is not an object.")
            self.event({"event": "response", "id": self.request_id, "method": method, "bytes": len(raw)})
            return response["result"]


def validate_inspection(value):
    if not isinstance(value, dict) or type(value.get("protocol")) is not int or value["protocol"] != 1:
        raise ValueError("Unsupported display inspection protocol.")
    uint(value.get("frame_id"), U64_MAX, "frame ID")
    if value["frame_id"] == 0:
        raise ValueError("Invalid frame ID.")
    uint(value.get("captured_at_ms"), U64_MAX, "capture time")
    if not isinstance(value.get("stage"), str) or not value["stage"] or len(value["stage"]) > 128:
        raise ValueError("Invalid display stage.")
    if value.get("recovery_blocked") is not False:
        raise ValueError("Display recovery is blocked or unknown.")
    frame = value.get("framebuffer")
    if not isinstance(frame, dict):
        raise ValueError("The framebuffer metadata is absent.")
    for name, expected in (("width", 960), ("height", 540), ("length", FRAME_BYTES)):
        if type(frame.get(name)) is not int or frame[name] != expected:
            raise ValueError(f"Unsupported framebuffer {name}.")
    if frame.get("format") != "gray4" or frame.get("packing") != "low-nibble-first":
        raise ValueError("Unsupported framebuffer format.")
    uint(frame.get("checksum"), 0xFFFFFFFF, "framebuffer checksum")
    if type(frame.get("committed_checksum")) is not int or frame["committed_checksum"] != frame["checksum"]:
        raise ValueError("The framebuffer does not match the committed image.")
    if frame.get("shutdown_complete") is not True:
        raise ValueError("Display shutdown is not confirmed.")
    recent = frame.get("most_recent_refresh")
    if (not isinstance(recent, dict) or recent.get("success") is not True
            or recent.get("shutdown_confirmed") is not True
            or type(recent.get("frame_checksum")) is not int or recent["frame_checksum"] != frame["checksum"]):
        raise ValueError("A successful refresh of this framebuffer is not confirmed.")
    return frame


def validate_chunk(value, frame_id, offset, length):
    if not isinstance(value, dict):
        raise ValueError("The framebuffer chunk is not an object.")
    for field, expected in (("frame_id", frame_id), ("offset", offset), ("length", length)):
        if type(value.get(field)) is not int or value[field] != expected:
            raise ValueError(f"The framebuffer chunk has an invalid {field}.")
    data = value.get("hex")
    if not isinstance(data, str) or len(data) != length * 2 or re.fullmatch(r"[0-9a-fA-F]+", data) is None:
        raise ValueError("The framebuffer chunk has invalid hexadecimal data.")
    return bytes.fromhex(data)


def checksum(data):
    result = 0x811C9DC5
    for byte in data:
        result = ((result ^ byte) * 0x01000193) & 0xFFFFFFFF
    return result


def capture_channel(rpc, save_snapshot=None):
    """Accept a frame only after all chunks and the final inspection agree."""
    before = rpc.call("display.inspect")
    if save_snapshot:
        save_snapshot("before", before)
    frame = validate_inspection(before)
    native = bytearray()
    for offset in range(0, FRAME_BYTES, CHUNK_BYTES):
        length = min(CHUNK_BYTES, FRAME_BYTES - offset)
        result = rpc.call("display.framebuffer.read", {"frame_id": before["frame_id"], "offset": offset, "length": length})
        native.extend(validate_chunk(result, before["frame_id"], offset, length))
    actual = checksum(native)
    if actual != frame["checksum"]:
        raise ValueError("The downloaded framebuffer checksum does not match.")
    after = rpc.call("display.inspect")
    if save_snapshot:
        save_snapshot("after", after)
    last = validate_inspection(after)
    if (before["frame_id"] != after["frame_id"] or before["stage"] != after["stage"]
            or frame["checksum"] != last["checksum"] or after["captured_at_ms"] < before["captured_at_ms"]):
        raise ValueError("The display changed during framebuffer capture.")
    return {"before": before, "after": after, "native": bytes(native)}


def portrait_pixels(native):
    if len(native) != FRAME_BYTES:
        raise ValueError("The native framebuffer has an invalid length.")
    # Match the shared PortraitDisplay rotation.
    pixels = bytearray(540 * 960)
    for y in range(960):
        shift = 4 if y & 1 else 0
        for x in range(540):
            pixels[y * 540 + x] = ((native[(539 - x) * 480 + y // 2] >> shift) & 15) * 17
    return bytes(pixels)


def png(pixels):
    if len(pixels) != 540 * 960:
        raise ValueError("The portrait image has an invalid length.")

    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)

    scanlines = b"".join(b"\0" + pixels[row * 540:(row + 1) * 540] for row in range(960))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 540, 960, 8, 0, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(scanlines)) + chunk(b"IEND", b""))


def histograms(pixels):
    if len(pixels) != 540 * 960:
        raise ValueError("The portrait image has an invalid length.")
    counts = [0] * 16
    for pixel in pixels:
        if pixel % 17:
            raise ValueError("The portrait image contains a non-gray4 value.")
        counts[pixel // 17] += 1
    return {"coordinate_space": "portrait-540x960", "levels": list(range(16)),
            "level_0": "black", "level_15": "white", "width": 540, "height": 960,
            "source": "intended framebuffer, not optical measurement", "counts": counts}


def capture(output_dir):
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=False)

    def save(name, value):
        (output_dir / f"{name}.json").write_text(json.dumps(value, indent=2) + "\n")

    with open("/tmp/paper-mesh-cdc.lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        port = matching_port(APPLICATION_IDENTITY)
        if port is None:
            raise ValueError("The specified application USB identity is absent.")
        with (output_dir / "events.jsonl").open("w") as events:
            def event(record):
                events.write(json.dumps(record, separators=(",", ":")) + "\n")
                events.flush()

            with open_application(port, APPLICATION_IDENTITY) as fd:
                result = capture_channel(Rpc(fd, event), save)
                check_port(port, APPLICATION_IDENTITY)
    pixels = portrait_pixels(result["native"])
    (output_dir / "native.bin").write_bytes(result["native"])
    (output_dir / "portrait.png").write_bytes(png(pixels))
    save("histogram", histograms(pixels))
    summary = {"status": "verified-framebuffer-capture", "directory": str(output_dir.resolve()),
               "frame_id": result["before"]["frame_id"], "stage": result["before"]["stage"],
               "checksum": checksum(result["native"]), "bytes": len(result["native"]),
               "optical_state_measured": False}
    save("summary", summary)
    return summary


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output_dir", type=Path, help="New directory for the verified capture.")
    args = parser.parse_args(argv)
    if os.environ.get("PAPER_MESH_DEVCONTAINER") != "1":
        parser.error("Run this script in the USB devcontainer.")
    try:
        result = capture(args.output_dir)
    except (OSError, ValueError) as error:
        print(json.dumps({"status": "capture-failed", "error": str(error), "reset_requested": False}), file=sys.stderr)
        return 1
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
