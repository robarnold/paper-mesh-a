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

Normal firmware supplies CDC logs and the [ROM download command](#enter-rom-download-mode).
The application accepts [read-only inspection commands](#inspect-the-held-framebuffer) during a completed diagnostic pattern.
Use BOOT and Reset when software entry is unavailable:

1. Hold BOOT.
2. Press and release Reset.
3. Release BOOT.
4. Confirm the ROM serial identity before flashing.

If the serial device does not return, disconnect USB and repeat the procedure after reconnecting with BOOT held.
Run espflash inside a container with the ROM serial device passed through.
The flash helper verifies the temporary software download request before flashing and preserves it through the transfer.
The [download procedure](#enter-rom-download-mode) defines the checks and the restart method.
It uses `--before no-reset` so the processor stays in the confirmed download session.
The [build guide](../firmware-build.md) defines the ELF and boot image artifacts.
No build recipe flashes hardware.

## Inspect the held framebuffer

Normal firmware permits inspection during its manual black, white, and gray checks.
See the [display check procedure](lilygo-t5-s3-pro-display.md#basic-display-check).
Other application screens reject these inspection methods.
The separate `system.bootloader` route remains available under its preparation checks.

Inspection does not refresh the panel, advance the test, or change display controls.
It does not wake the PMIC or read its registers again.
The runtime must own the idle display, with completed shutdown and no retained recovery fault.
One request can wait in the queue while the preceding response completes.

Requests use JSON-RPC 2.0 with one object per newline and at most 512 bytes before the newline.
An ID must be an integer or a string of at most 64 bytes. Null IDs are invalid for inspection.
Notifications perform no read and receive no response. Batch requests are unsupported.
Unknown envelope fields and invalid parameters produce errors.

| Method | Parameters | Result |
| --- | --- | --- |
| `display.inspect` | Absent or `{}` | Current hold, framebuffer metadata, and retained refresh measurements |
| `display.framebuffer.read` | Exactly `frame_id`, `offset`, and `length` | One native framebuffer chunk |

For a metadata request, send:

```json
{"jsonrpc":"2.0","id":1,"method":"display.inspect"}
```

The success envelope is `{"jsonrpc":"2.0","id":1,"result":{...}}`.
The `result` object has these fields:

| Field | Type and meaning |
| --- | --- |
| `protocol` | Integer `1` |
| `stage` | Current completed pattern: `black`, `white`, or `gray` |
| `frame_id` | Positive integer that identifies this idle hold within the current startup |
| `captured_at_ms` | Unsigned monotonic uptime in milliseconds |
| `recovery_blocked` | Boolean, false for a successful response |
| `framebuffer` | Object described below |

Each new hold gets a new `frame_id`, even if its pixels match an earlier image.
The runtime revokes the hold before it changes the frame or exits the check.
The ID does not persist across resets.
The runtime checks the hold and USB session before it serves each request.
It retains display ownership until the bounded response write completes.

### Framebuffer metadata

`display.inspect` computes a fresh checksum and reads the PCA9535 port 1 registers.
It reports earlier PMIC measurements separately.

| Field | Type and meaning |
| --- | --- |
| `width`, `height` | Native dimensions, integers `960` and `540` |
| `format` | String `gray4` |
| `packing` | String `low-nibble-first` |
| `length` | Integer `259200`, the native byte count |
| `checksum` | Unsigned 32-bit FNV-1a checksum of the current native bytes |
| `committed_checksum` | Unsigned 32-bit checksum of the committed history, or null when unavailable |
| `shutdown_complete` | Boolean derived from the shutdown preconditions and current control levels |
| `sampled_at_ms` | Unsigned monotonic uptime in milliseconds |
| `expander_port1` | Current expander readings and the cached output value |
| `most_recent_refresh` | Most recent physical refresh record, or null before the first attempt |

Each byte contains two native pixels. The low nibble contains the even native X coordinate.
The high nibble contains the next pixel. Gray level 0 is black and level 15 is white.
The portrait image is 540 × 960 after the shared rotation.

`expander_port1` contains raw byte fields `input`, `output`, `configuration`, and `inversion` from the expander.
Reading the PCA9535 input port can acknowledge its interrupt comparison.
Its `cached_output` byte comes from the driver's confirmed output cache.
Its `controls_off` Boolean checks the display output directions, latch bits, and input levels after polarity correction.
These checks confirm digital controls. They do not measure analog supply voltages.

### Retained refresh measurements

`most_recent_refresh` has these fields:

| Field | Type and meaning |
| --- | --- |
| `epoch` | Unsigned 32-bit attempt counter |
| `started_ms`, `finished_ms` | Unsigned uptime in milliseconds. `finished_ms` is null until completion |
| `mode` | String `full` or `partial` |
| `frame_checksum` | Unsigned 32-bit checksum of the attempted image |
| `success` | Boolean result, or null before completion |
| `shutdown_confirmed` | Boolean shutdown result, or null before completion |
| `pmic` | Retained power measurements, or null when unavailable |

A new refresh attempt clears the previous attempt's result and PMIC record.
An unchanged update that needs no physical refresh retains the previous physical refresh record.
The host also checks that its checksum matches the current image.

The `pmic` object contains values from existing reads during that power sequence:

| Field | Type and meaning |
| --- | --- |
| `source` | String `existing reads during refresh; not live` |
| `attempt` | Unsigned 32-bit power attempt counter |
| `started_ms`, `ready_ms`, `shutdown_ms` | Unsigned uptime in milliseconds. The last two fields can be null |
| `revision` | Controller revision byte |
| `temperature` | Signed thermistor reading in degrees Celsius |
| `enable_start`, `enable_off` | ENABLE register bytes |
| `vcom1`, `vcom2` | VCOM register bytes |
| `sequence` | Four bytes in UPSEQ0, UPSEQ1, DWNSEQ0, DWNSEQ1 order |
| `faults_before`, `faults_active` | Two bytes in INT1, INT2 order, before activation and after VCOM activation |
| `power_good_start`, `power_good_active`, `power_good_off` | PG register bytes from the named checkpoints |

Register values and array entries are null when a required read or readback check did not complete.
Reading this record does not clear PMIC fault flags.
VCOM registers report the programmed value. They do not report the measured voltage at the panel.
PG and fault bits report the controller's threshold checks, not full voltage waveforms.

### Download one held frame

Use the `frame_id` from the metadata response for every chunk request:

```json
{"jsonrpc":"2.0","id":2,"method":"display.framebuffer.read","params":{"frame_id":1,"offset":0,"length":512}}
```

`offset` is an unsigned native byte offset. `length` is an integer from 1 through 512.
The sum of `offset` and `length` must not exceed 259200.
The success result contains exactly `frame_id`, `offset`, `length`, and `hex`.
The `hex` string contains two hexadecimal characters per byte, in native byte order.
The final chunk in a 512-byte transfer has 128 bytes.

Errors use `{"jsonrpc":"2.0","id":2,"error":{"code":-32001,"message":"Frame changed"}}` for a different requested frame.
Code `-32000` covers an unavailable hold, pending request, connection change, recovery fault, or backend read failure.
Standard codes `-32700`, `-32600`, `-32601`, and `-32602` cover parsing, request, method, and parameter errors.
An expired hold can discard a queued request. The host must also handle a response timeout.

Connect USB during an unchanged hold. Keep BOOT untouched during capture.
Run the helper inside the USB devcontainer with a new output directory:

```sh
python3 scripts/inspect_display.py backups/display-inspection
```

The helper requires identity `cafe:4010`, serial `paper-mesh-prototype`, and the shared `/tmp/paper-mesh-cdc.lock`.
It uses one exclusive application connection for the metadata, 507 chunks, and final metadata response.
Each request has a five-second deadline. The complete capture has a 180-second deadline.
Response lines have an 8192-byte limit. Each request permits at most 65536 received bytes, including unrelated logs.
The helper sends only the two inspection methods and does not retry a failed capture.

The helper validates every chunk ID, offset, length, and hexadecimal value.
It computes FNV-1a over all native bytes and checks the reported checksum.
The final inspection must have the same frame ID, stage, and checksum, with confirmed shutdown and no recovery block.
Both inspections must match committed history and a successful physical refresh with confirmed shutdown.
If BOOT advances the test, these checks fail or the old request times out.
The helper then stops without publishing the native image or preview.

A successful capture saves `before.json`, `after.json`, `native.bin`, `portrait.png`, `histogram.json`, `events.jsonl`, and `summary.json`.
The event log contains request and response metadata. It excludes framebuffer payloads and unrelated log contents.
The histogram counts all 16 gray levels across the complete framebuffer.
It does not assume defect locations or comparison regions.
USB can remain connected between the manual steps.

The capture describes the intended software image and reported controls.
The panel provides no documented pixel readback, charge measurement, or row-health report through this interface.
A matching image does not prove the displayed shade, panel condition, or electrical waveform.

## Enter ROM download mode

Normal and diagnostic firmware accept an explicit JSON-RPC request on CDC:

```json
{"jsonrpc":"2.0","id":1,"method":"system.bootloader"}
```

Send a newline after the request. The command accepts no parameters.
Notifications and null request IDs cannot reset the device.
This maintenance command does not require a diagnostic feature.
The CDC receiver participates in service preparation for light sleep.

The runtime retains its application future but stops polling it during request processing.
It rejects queued or active display refreshes, power transitions, active recovery deadlines, and retained display faults.
It parks core 1 at its cooperative stop point and verifies the display controls under one bus lock.
Verification reads the expander latch, direction, inversion, and physical input values. It does not change display voltages or calibration.
Failed preparation resumes the same application future. The command does not interrupt a scan or clear a recovery fault.

After preparation, the response contains `accepted: true` and `mode: "rom-download"`.
The runtime waits for the complete CDC response write before it disconnects USB and resets into the ROM downloader.
A connection change or failed response write cancels the reset and releases the parked core.
The host must also confirm ROM enumeration. An acknowledgement alone does not prove that download mode started.

Run the helper inside the USB devcontainer:

```sh
python3 scripts/enter_download_mode.py --rom-serial '<expected-ROM-serial>'
```

The helper checks the application identity before it sends one request.
It keeps DTR asserted until the application disconnects, then closes the old port before it waits for ROM enumeration.
It then requires USB identity `303a:1001` with the specified ROM serial.
Both steps share a ten-second deadline after acknowledgement.
It sends no retry, flash command, or hardware reset after an error or uncertain result.
The application serial defaults to `paper-mesh-prototype`. Use `--application-serial` to specify another identity.

The reset uses volatile control registers. It does not write eFuses or flash.
The chip must permit software download through USB Serial/JTAG and use the expected internal PHY selection.
Manual BOOT/Reset remains necessary for older firmware, an unresponsive application, or a rejected recovery state.

After download entry, flash the checked ELF with a new output directory:

```sh
python3 scripts/flash_firmware.py target/artifacts/paper-mesh-firmware.elf backups/firmware-flash --rom-serial '<expected-ROM-serial>'
```

Use the actual ELF path from the [build output](../firmware-build.md).
The helper requires the USB devcontainer and the shared `/tmp/paper-mesh-cdc.lock`.
It does not enter download mode or start a display check.
It writes ROM preparation, flash output, and any required finalization evidence into the output directory.
A completed flash does not confirm application startup. Check enumeration and startup logs separately.

`scripts/flash_firmware.py` prepares each flash with `scripts/prepare_rom_flash.py`.
Preparation verifies the ROM USB identity and the ESP32-S3 chip identity without register writes.
For software download, flashing preserves `FORCE_DOWNLOAD_BOOT` and returns the stub to ROM after a successful transfer.
The helper then verifies ROM identity, chip identity, and the retained request again.
ROM readiness permits up to four read-only SYNC attempts within ten seconds. Protocol errors stop this step.
It clears only bit 0 of `RTC_CNTL_OPTION1`, verifies the readback, and requests an RTC chip reset.
Application USB and startup logs must confirm the return. A reset request does not prove that startup completed.
For manual download with no retained request, the helper selects `hard-reset` or `watchdog-reset` from the latched boot straps.
A transport, identity, readback, or flash error stops the procedure after synchronization. Register writes and reset requests never retry.
The helper does not use another board.
The register operations follow Espressif's [ROM serial protocol][rom-protocol].

## Verification limits

Linux driver binding, libinput classification, and startup CDC logging have physical verification.
Physical BOOT/Reset recovery and flashing through USB Serial/JTAG have verification.
Software download entry, flashing, and return to application startup have physical verification on the target board.
This verifies one complete automatic cycle. Repeated-cycle reliability and rejection paths still require physical checks.
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

[usb-device]: https://docs.espressif.com/projects/esp-usb/en/latest/esp32s3/usb_device.html
[rom-protocol]: https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/serial-protocol.html
