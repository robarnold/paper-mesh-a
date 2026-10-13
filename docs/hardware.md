# LILYGO T5 S3 E-Paper Pro

The initial target is the **H752-01 full Pro with LoRa**, not the older H752 or
the Lite model. This profile comes from LILYGO's repository at commit
[`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
Confirm the revision and fitted components when the device arrives.
The [hardware reference index](vendor/lilygo/t5-s3-pro/README.md) links to the
vendor's schematics, datasheets, pin maps, and mechanical drawings.
Panel scan geometry, waveform/driver limitations, and the evidence needed for
application performance budgets are documented in
[display-constraints.md](display-constraints.md).

## Build profile

| Item | Configuration |
| --- | --- |
| SoC | ESP32-S3, dual Xtensa LX7 |
| Rust target | `xtensa-esp32s3-none-elf` |
| Flash | 16 MB, QIO, 80 MHz |
| PSRAM | 8 MB OPI; not enabled or allocated by this bootstrap |
| Application CPU clock | 80 MHz |
| Executor | Embassy via esp-rtos on core 0; core 1 not started |
| Application logging | USB Serial/JTAG only |
| Display | ED047TC1, 960 × 540, 4.7-inch grayscale e-paper |
| Radio | SX1262; driver and protocol not selected yet |

Flash and PSRAM specifications are in the vendor's
[board definition](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/boards/T5-ePaper-S3.json).
The [vendor README](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/README.md)
documents the models and component variations. Touch and GPS parts can vary;
driver selection needs confirmation against the actual unit.

## Pins for later driver bring-up

These are ESP32 GPIO numbers from the vendor's
[factory pin definitions](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/utilities.h).

| Interface | Pins |
| --- | --- |
| I2C | SDA 39, SCL 40 |
| Shared SPI | SCK 14, MOSI 13, MISO 21 |
| SX1262 | CS 46, IRQ 10, reset 1, busy 47 |
| microSD | CS 12 |
| Touch | interrupt 3, reset 9 |
| GPS UART | MCU RX 44, MCU TX 43 |
| Frontlight enable | 11 |
| RTC interrupt | 2 |
| PCA9535 interrupt | 38 |
| Boot button | 0 |

GPIO 43/44 are also UART0's usual pins. `esp-println` has automatic transport
selection disabled so application and panic logs never fall back to the GPS
UART. ROM/bootloader logging is separate from this application setting.

The display's parallel data/control signals and power sequencing should be
implemented together using the vendor's
[complete pin table](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pin_define.md).
Several display power/control signals go through the PCA9535 expander and the
TPS65185 supply. The board also includes a BQ25896 charger and BQ27220 fuel gauge;
these require deliberate configuration before claiming battery operation is
managed.

## First hardware checks

1. Confirm the board revision and USB/serial passthrough into the devcontainer.
2. When flashing is requested, preserve the factory firmware before replacing it.
3. Verify the boot banner and advancing heartbeat over USB, including recovery
   after reset and USB disconnect/reconnect.
4. Bring up I2C identification, battery telemetry, and controlled peripheral power
   before adding display/touch or radio traffic.

The firmware currently leaves board peripherals uninitialized; that does not
guarantee they are powered off. Timer waits yield to the scheduler, but automatic
light sleep and deep sleep are not configured. Measure power on the real board
before drawing conclusions about battery life. MeshCore integration, multicore
work allocation, persistent storage, and an OTA partition/update design remain
separate steps.
