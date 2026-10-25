#!/usr/bin/env python3
"""Flash one identified board from ROM download mode and preserve the flash record."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys

from device_usb import find_port


def flash(image, rom_serial, record):
    from prepare_rom_flash import finalize, prepare

    if not rom_serial:
        raise ValueError("Flashing requires --rom-serial with the expected board serial.")
    if image.read_bytes()[:4] != b"\x7fELF":
        raise ValueError("The flash input must be the checked firmware ELF.")
    port = find_port(("303a", "1001", rom_serial))
    if port is None:
        raise ValueError("The specified board is not in download mode. Hold BOOT, press and release Reset, then release BOOT.")
    prepared = prepare(port, rom_serial)
    (record / "rom-preparation.json").write_text(json.dumps(prepared, indent=2) + "\n")
    if find_port(("303a", "1001", rom_serial)) != port:
        raise ValueError("The ROM identity changed after preparation. No flash was attempted.")
    command = ["espflash", "flash", "--port", str(port), "--chip", "esp32s3",
               "--before", "no-reset", "--after", prepared["after"], "--baud", "921600",
               "--flash-size", "16mb", "--flash-mode", "dio", "--flash-freq", "80mhz",
               "--non-interactive", "--skip-update-check", str(image)]
    print("Flashing the identified board after read-only ROM checks.", flush=True)
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120)
    (record / "flash.log").write_text(result.stdout)
    print(result.stdout, end="", flush=True)
    if result.returncode:
        raise ValueError("Flashing failed. See flash.log.")
    if prepared["finalize_required"]:
        try:
            finalized = finalize(rom_serial, prepared["option1_before"])
        except (OSError, ValueError, RuntimeError) as error:
            (record / "rom-finalization-error.json").write_text(json.dumps({
                "error": str(error), "outcome": "unconfirmed", "retried": False,
                "reset_attempted": getattr(error, "reset_attempted", None),
                "reset_acknowledged": getattr(error, "reset_acknowledged", None),
                "phase": getattr(error, "phase", None),
                "sync_attempts": getattr(error, "sync_attempts", None),
            }, indent=2) + "\n")
            raise
        (record / "rom-finalization.json").write_text(json.dumps(finalized, indent=2) + "\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path, help="Checked firmware ELF.")
    parser.add_argument("record", type=Path, help="New directory for flash evidence.")
    parser.add_argument("--rom-serial", required=True, help="Exact serial of the expected ROM USB device.")
    args = parser.parse_args(argv)
    if os.environ.get("PAPER_MESH_DEVCONTAINER") != "1":
        parser.error("Run this script in the USB devcontainer.")
    try:
        args.record.mkdir(parents=True, exist_ok=False)
        with open("/tmp/paper-mesh-cdc.lock", "w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            flash(args.image, args.rom_serial, args.record)
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(json.dumps({"status": "flash-unconfirmed", "error": str(error), "retried": False}), file=sys.stderr)
        return 1
    print(json.dumps({"status": "flash-complete", "record": str(args.record.resolve()),
                      "application_startup_verified": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
