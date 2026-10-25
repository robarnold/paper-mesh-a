from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import prepare_rom_flash
import flash_firmware as module


class FlashFirmwareTests(unittest.TestCase):
    def test_flash_rejects_missing_or_wrong_board_without_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            image = path / "test.elf"
            image.write_bytes(b"\x7fELFfixture")
            with patch.object(module, "find_port", return_value=None), patch.object(module.subprocess, "run") as run:
                for serial in [None, "different-board"]:
                    with self.subTest(serial=serial), self.assertRaises(ValueError):
                        module.flash(image, serial, path)
                run.assert_not_called()

    def test_flash_requires_verified_rom_preparation_and_preserves_its_reset_choice(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            image = path / "test.elf"
            image.write_bytes(b"\x7fELFfixture")
            port = Path("/dev/ttyACM0")
            for reset in ("hard-reset", "watchdog-reset"):
                prepared = {"after": reset, "strap": 8 if reset == "hard-reset" else 0,
                            "option1_before": 0, "option1_after": 0, "finalize_required": False}
                with self.subTest(reset=reset), patch.object(module, "find_port", return_value=port), \
                        patch.object(prepare_rom_flash, "prepare", return_value=prepared) as prepare, \
                        patch.object(prepare_rom_flash, "finalize") as finalize, \
                        patch.object(module.subprocess, "run", return_value=SimpleNamespace(stdout="done", returncode=0)) as run, \
                        patch("builtins.print"):
                    module.flash(image, "board", path)
                    prepare.assert_called_once_with(port, "board")
                    command = run.call_args.args[0]
                    self.assertEqual(command[command.index("--before") + 1], "no-reset")
                    self.assertEqual(command[command.index("--after") + 1], reset)
                    finalize.assert_not_called()
            for error, ports in ((ValueError("readback failed"), [port]), (None, [port, None])):
                with self.subTest(error=error), patch.object(module, "find_port", side_effect=ports), \
                        patch.object(prepare_rom_flash, "prepare", side_effect=error, return_value=prepared), \
                        patch.object(module.subprocess, "run") as run, self.assertRaises(ValueError):
                    module.flash(image, "board", path)
                run.assert_not_called()

    def test_software_download_flag_clears_only_after_successful_flash(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            image = path / "test.elf"
            image.write_bytes(b"\x7fELFfixture")
            prepared = {"after": "no-reset", "strap": 3,
                        "option1_before": 0xA5, "option1_after": 0xA5, "finalize_required": True}
            events = []

            def flashed(command, **_kwargs):
                self.assertEqual(command[command.index("--after") + 1], "no-reset")
                events.append("flash")
                return SimpleNamespace(stdout="verified", returncode=0)

            def finalized(serial, before):
                self.assertEqual(events, ["flash"])
                self.assertEqual((serial, before), ("board", 0xA5))
                events.append("finalize")
                return {"option1_before": 0xA5, "option1_after": 0xA4, "reset_requested": True}

            with patch.object(module, "find_port", return_value=Path("/dev/ttyACM0")), \
                    patch.object(prepare_rom_flash, "prepare", return_value=prepared), \
                    patch.object(module.subprocess, "run", side_effect=flashed), \
                    patch.object(prepare_rom_flash, "finalize", side_effect=finalized), \
                    patch("builtins.print"):
                module.flash(image, "board", path)
            self.assertEqual(events, ["flash", "finalize"])
            self.assertTrue((path / "rom-finalization.json").exists())

    def test_failed_flash_preserves_download_state_and_skips_finalization(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            image = path / "test.elf"
            image.write_bytes(b"\x7fELFfixture")
            prepared = {"after": "no-reset", "strap": 3,
                        "option1_before": 1, "option1_after": 1, "finalize_required": True}
            with patch.object(module, "find_port", return_value=Path("/dev/ttyACM0")), \
                    patch.object(prepare_rom_flash, "prepare", return_value=prepared), \
                    patch.object(module.subprocess, "run", return_value=SimpleNamespace(stdout="failed", returncode=1)), \
                    patch.object(prepare_rom_flash, "finalize") as finalize, \
                    patch("builtins.print"), self.assertRaisesRegex(ValueError, "Flashing failed"):
                module.flash(image, "board", path)
            finalize.assert_not_called()

    def test_failed_finalization_is_recorded_without_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            image = path / "test.elf"
            image.write_bytes(b"\x7fELFfixture")
            prepared = {"after": "no-reset", "strap": 3,
                        "option1_before": 1, "option1_after": 1, "finalize_required": True}
            with patch.object(module, "find_port", return_value=Path("/dev/ttyACM0")), \
                    patch.object(prepare_rom_flash, "prepare", return_value=prepared), \
                    patch.object(module.subprocess, "run", return_value=SimpleNamespace(stdout="verified", returncode=0)) as run, \
                    patch.object(prepare_rom_flash, "finalize", side_effect=ValueError("readback failed")) as finalize, \
                    patch("builtins.print"), self.assertRaisesRegex(ValueError, "readback failed"):
                module.flash(image, "board", path)
            run.assert_called_once()
            finalize.assert_called_once()
            self.assertIn("readback failed", (path / "rom-finalization-error.json").read_text())


if __name__ == "__main__":
    unittest.main()
