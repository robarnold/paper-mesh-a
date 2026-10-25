"""Find a USB CDC device by its complete USB identity."""

from pathlib import Path


def identity(tty_entry):
    for parent in (tty_entry / "device").resolve().parents:
        try:
            return tuple((parent / name).read_text().strip()
                         for name in ("idVendor", "idProduct", "serial"))
        except OSError:
            continue
    return None


def find_port(expected):
    matches = [Path("/dev") / p.name for p in Path("/sys/class/tty").glob("ttyACM*")
               if identity(p) == expected]
    if len(matches) > 1:
        raise ValueError("More than one matching USB device is connected.")
    return matches[0] if matches else None
