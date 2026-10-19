# LILYGO touch input and interrupts

[Backend profile](lilygo-t5-s3-pro.md)

## Current touch adapter

[`crates/gt911`](../../crates/gt911/src/lib.rs) implements the GT911 report protocol.
The [crate guide](../../crates/gt911/README.md) records protocol references and test scope.
[`firmware/src/touch.rs`](../../firmware/src/touch.rs) adapts reports to the shared touch contract.
The driver probes both documented addresses and checks the product identity.
If neither probe succeeds, it applies the documented reset sequence and selects address `0x5D`.
It records the existing configuration through USB without configuration writes.

Reports must have the ready flag before the driver reads coordinates.
The driver acknowledges each report after the required reads succeed.
It emits each ready contact position with its ID, then a release with the final position.
The driver does not recognize taps or remove movement reports.
The driver tracks up to five contact IDs, independent of their order in a report.
Invalid coordinates, duplicate IDs, excess contacts, palm reports, I2C failures, and queue overflow cancel all contacts.
Recovery waits for all contacts to lift.
Ready reports also supply Home-key state through `HaveKey` in register `0x814E`.
The adapter exposes that state as a hardware button, independent of coordinates.
Pen input stays unimplemented.

Firmware samples electrical buttons every 10 ms with a 20 ms debounce interval.
GT911 ready reports supply the controller's Home-key level without another debounce interval.
The first key observation synchronizes state. A key held at startup does not create a press event.

[Goodix Programming Guide Rev.10][gt911-guide], sections 3.3, 4.2, and 5, defines identity, reset timing, and report handling.
The adapter converts the fitted portrait coordinates through the shared native-to-portrait transform.

## Concurrent display and touch

The display worker runs on core 1 with an 8 KiB stack.
The runtime transfers exclusive panel ownership through a channel before each refresh.
Completion returns ownership and any error to core 0.
The two existing PSRAM buffers remain sufficient. No frame copies occur during ownership transfer.

GPIO3 uses the existing GT911 low-level interrupt mode from register `0x804D`.
The GPIO handler records the interrupt time, disables that interrupt, and signals the touch worker.
The worker uses a priority-2 interrupt executor on core 0.
It reads and acknowledges the report, then enables the GPIO interrupt after a 1 ms delay.
A low level that remains pending triggers another interrupt.
The driver does not change the controller configuration or poll it during idle periods.

An asynchronous mutex protects I2C0. Interrupts remain enabled during bus transactions.
Display control operations release the mutex after each transaction.
Pixel transfers use LCD DMA and RMT without the I2C mutex.
Touch reports therefore proceed during display scans.

The touch queue holds 32 complete frames. Overflow cancels input and increments an error counter.
Each frame has an interrupt timestamp and up to ten events, including releases and replacement contacts.

USB reports the core assignment, interrupt times, service duration, bus wait, and events received during refresh.
Service duration starts at GPIO handler entry and ends after the report read and acknowledgment attempt.
It excludes interrupt latency before handler entry.
The native burst test verifies all 40 taps across multiple pending refreshes and compares the final image pixel by pixel.

The implementation follows esp-rtos 0.4.0
[`start_second_core`](https://github.com/esp-rs/esp-hal/blob/63e97be553f435f29171f7ae5f37a41ad737a75a/esp-rtos/src/lib.rs#L440-L458)
and [`InterruptExecutor`](https://github.com/esp-rs/esp-hal/blob/63e97be553f435f29171f7ae5f37a41ad737a75a/esp-rtos/src/embassy/mod.rs#L308-L419).
The first API pins the display thread to core 1.
The second API starts touch service on the core that installs its software interrupt.

## Verification limits

Physical checks established single-contact input and rapid taps during concurrent display refresh.
The fitted controller configuration can limit reporting below the protocol maximum.
Firmware logs that limit from register `0x804C` and preserves it.
Physical checks established two-contact tracking and two-finger scrolling through the USB touchpad.
Native protocol tests cover five contacts and changes in report order.
Edge calibration, maximum input rate, and latency before GPIO handler entry remain unverified.
Unread GT911 reports can delay later contact changes until the host acknowledges the report.
Touch service must therefore continue during display scans.

## GT911 Home key configuration and lower bezel

The review date is **2026-10-13**.
The controller has configurable touch-key recognition.
Extra usable bezel area stays **unconfirmed** for this panel.

[Goodix's programming guide, Rev.10][gt911-guide], section 3.2, gives information about these
registers:

| Registers | Meaning |
| --- | --- |
| `0x804E` bit 0 | Touch-key enable |
| `0x8093`–`0x8096` | Four key positions. Zero disables a key. Values that are all multiples of eight identify independent-key designs. |
| `0x8097` | Lower nibble: one-sided active extent (0–15). Upper nibble: baseline updates during a held touch. |
| `0x8098`–`0x809B` | Key press and release thresholds, sensitivity |
| `0x805B`–`0x805C` | Crop settings for four borders when the sensor grid extends beyond the visible display |

Guide pages 5–6, 9, and 12–13 give information about these settings.
The guide does not give pixel or millimeter units for key-area values.
Section 3.3, page 17, defines `HaveKey` as pressed or released.
Interpretation must use a valid, ready report.

[Goodix's datasheet][gt911-datasheet], section 5.4, gives information about up to four keys.
This section is on printed page 8, or PDF page 9.
The keys use a separate transmit channel and shared receive channels.
The description includes a separate FPC electrode design.

This design does not establish an arbitrary X/Y rectangle across the lower bezel.
Configuration cannot add electrodes to the panel.
A larger key response area can not necessarily provide useful X/Y tracking.

The [LILYGO factory initialization][factory-touch] installs the callback and
selects low-level interrupt reports.
[Driver initialization][gt911-init] reads the existing controller settings.
[Interrupt setup][gt911-irq] changes interrupt bits and updates the configuration
checksum. It does not install a board-specific key-position or area table.
Thus, reviewed factory and touch examples do not identify the shipped register
values.

The [`setMaxCoordinates` helper][touch-coordinate-helper] in newer examples sets
host mirroring bounds. It does not expand the sensor area.
The board schematic shows the touch module's host connection.
It does not show electrode layout.
The reviewed references contain no fitted-panel electrode drawing or configuration
dump.

Two possibilities are subjects for hardware tests.
Existing key electrodes might permit a different recognition area.
The main sensor matrix might include more area outside the configured
border. Neither possibility establishes an extra input surface.

When hardware setup is requested, use this sequence:

1. Read the existing `0x8047`–`0x8100` configuration.
2. Save the configuration before any change.
3. Record the controller identity, firmware identity, and module ID at `0x814A`.
4. Inspect the key, border, and channel-map settings together.
5. Record ready touch and key reports during taps across the lower bezel.
6. Include press, release, the marked Home area, and adjacent positions.
7. Keep raw sensor observations separate from rotated application coordinates.
8. If readback and panel evidence support an experiment, change one setting from the saved baseline.
9. Apply the checksum and update procedure in guide section 4.4.
10. Compare the response map with the baseline.
11. Check ordinary screen accuracy.

The vendor's [`loadConfig`/`dumpRegister` methods][gt911-config] provide a readback
example. Reset does not necessarily restore initial settings.
Until measurements establish extra area, Home stays a logical key and extra
bezel controls stay unavailable.
The current adapter exposes configured Home-key reports. Extra bezel area remains outside the input model.

[factory-touch]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/main.cpp#L328-L351
[gt911-init]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L546-L645
[gt911-irq]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L275-L292
[gt911-config]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L425-L485
[touch-coordinate-helper]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvInterface.cpp#L77-L99
[gt911-guide]: https://www.lcd-module.de/fileadmin/eng/pdf/zubehoer/GT911_Programming_Guide_Rev.10.pdf
[gt911-datasheet]: https://www.olimex.com/Products/OLinuXino/LCD/_resources/GT911_Datasheet.pdf
