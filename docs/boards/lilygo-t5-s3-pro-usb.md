# LILYGO USB touchpad and diagnostics

[Backend profile](lilygo-t5-s3-pro.md) · [Host contract](../services/touchpad.md)

## Transport

The firmware uses the ESP32-S3 USB OTG controller through esp-hal and Embassy USB.
GPIO20 is D+ and GPIO19 is D−.
The internal PHY serves a composite device with CDC serial and HID touchpad interfaces.
The HID interface stays enumerated while the application mode is inactive. It sends no contacts in that state.

The prototype uses VID `0xCAFE` and PID `0x4010`.
These values are private development identifiers, not an allocated product identity.
The serial string is `paper-mesh-prototype`.
Shipping devices need allocated identifiers and a unique serial identity.

The HID descriptor uses the Digitizers Touch Pad usage and five finger collections.
Each contact has a tip flag, confidence flag, ID, and absolute X/Y coordinates.
The report also has Contact Count and Scan Time in 100 µs units.
Features supply Contact Count Maximum, native Input Mode, and an empty certification payload.
That payload selects Linux's generic Win8 multitouch class. It does not claim Windows certification.
Linux's `hid-multitouch` driver and libinput recognize the prototype without a custom host driver.
Windows and macOS support remain unverified.

The backend uses portrait coordinates and the panel's 58.32 × 103.68 mm active area.
The [panel specification][panel], section 2, supplies these dimensions.
Wire coordinates use 16 units per pixel to reduce Linux's integer resolution error.
This scale does not increase sensor precision.
The descriptor rounds physical dimensions to tenths of a millimeter.
There is no physical click button. Linux supplies tapping, pointer acceleration, and gesture policy.

`paper-mesh-hid-touchpad` encodes complete reports without an application dependency.
Release entries retain the last position and ID.
When releases and replacement IDs exceed five entries, the encoder sends two complete reports.
The first report releases old contacts. The second report supplies the current contacts.
Scan Time advances between reports, including reports within one millisecond.

## Scheduling and cancellation

The input queue retains 16 complete contact states.
The HID endpoint uses an 8 ms poll interval and a 64-byte packet limit.
Each input report fits in one 34-byte packet.
Held contacts repeat after 16 ms without a new frame to prevent Linux's 100 ms contact timeout.
Idle sessions with no contacts send no periodic reports.
A write has a 100 ms deadline. Failure cancels the session and restarts USB.
Queue generations prevent delivery of reports from an earlier connection.
A failed write from an earlier generation cannot restart the current connection.

Mode exit, explicit cancellation, transport failure, and resume detach the composite device.
Resume causes detachment only after a configured connection enters the suspended state.
The adapter restores the USB clocks and sets `DCTL.SFTDISCON` before it disables the Embassy device state.
The resolved Synopsys driver, version 0.4.0, does not implement peripheral shutdown in its `Bus::disable()` method.
The adapter clears `DCTL.SFTDISCON` after 250 ms to permit re-enumeration.
This operation also disconnects CDC serial. Serial clients must reconnect.
It removes host gesture state that release reports alone might retain.
A held contact cannot start input again until all contacts lift.

The runtime drains queued touch frames before application button actions.
Function and Home exit the mode through application bindings.
Screen movement does not request display refreshes.

## Diagnostics and recovery

USB OTG and USB Serial/JTAG share the internal PHY.
See Espressif's [USB device stack documentation][usb-device] for this restriction.
Application logs therefore use CDC serial after startup.
Logging retains 32 lines with a 192-byte limit per line. A full queue drops new lines.
Logging never waits for a connected terminal in application or interrupt code.
UART logging remains unavailable because the board connects that UART path to GNSS.
The panic handler still uses Serial/JTAG. Panic text is unavailable after the PHY switches to OTG.
Before the panic handler prints, a hook saves up to 12 return addresses in persistent RTC memory.
The next startup sends those addresses through the CDC log queue if the memory retains its contents.
Decode the addresses with the ELF from the firmware that panicked.
Power loss can erase this evidence. The hook does not detect a stall without a panic.

CDC supplies logs only. It does not accept commands.
Software recovery from USB OTG to the ROM downloader remains unsupported.
Use BOOT and Reset before flashing:

1. Hold BOOT.
2. Press and release Reset.
3. Release BOOT.
4. Confirm the ROM serial identity before flashing.

If the serial device does not return, disconnect USB and repeat the procedure after reconnecting with BOOT held.
Run espflash inside a container with the ROM serial device passed through.
Use `--before usb-reset --after watchdog-reset` with the selected port.
The [build guide](../firmware-build.md) defines the ELF and boot image artifacts.
No build recipe flashes hardware.

## Verification limits

Linux driver binding, libinput classification, and startup CDC logging have physical verification.
Physical BOOT/Reset recovery and flashing through USB Serial/JTAG have verification.
Native tests verify contact reports, releases, mode exit, backpressure, and reconnect suppression.
Physical pointer movement, two-finger scrolling, and exit through Function and Home have verification.
USB control requests succeed after both exit paths.
Button debounce under repeated presses, suspend recovery, and retained panic diagnostics require further physical checks.
CDC log delivery after mode exit remains unverified.
Five-contact decoding does not establish five-contact reporting from the fitted controller configuration.
The [touch profile](lilygo-t5-s3-pro-touch.md) records that distinction.

The HAL currently forces its internal VBUS-valid input high.
The board does not supply a verified, independent USB VBUS sense input to this adapter.
Suspend and reset events invalidate sessions. They do not establish physical cable presence.
Battery-only attachment behavior and USB power compliance remain unverified.
The USB descriptor declares self power and a 500 mA limit.
This descriptor does not control charger current settings.

[panel]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/ED047TC1.pdf
[usb-device]: https://docs.espressif.com/projects/esp-usb/en/latest/esp32s3/usb_device.html
