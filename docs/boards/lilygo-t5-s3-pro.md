# LILYGO T5 S3 E-Paper Pro backend profile

This profile maps one hardware target to the portable
[software architecture](../architecture.md) and
[service contracts](../device-interfaces.md).
Its chips, controls, dimensions, and wake restrictions apply to this backend.
Other targets can have different characteristics.

The initial target is the **H752-01 full Pro with LoRa**.
The older H752 and Lite models are different targets.
This profile uses LILYGO sources at commit
[`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
The inventory review used the schematic and examples on **2026-10-12**.

Before use of another unit, confirm its revision and fitted components.
The [hardware reference index](../vendor/lilygo/t5-s3-pro/README.md) links to
schematics, datasheets, pin maps, and mechanical drawings.
[Display backend evidence](lilygo-t5-s3-pro-display.md) gives information about scan geometry,
waveform limits, driver limits, and necessary performance measurements.

## Build profile

| Item | Configuration |
| --- | --- |
| SoC | ESP32-S3, dual Xtensa LX7 |
| Rust target | `xtensa-esp32s3-none-elf` |
| Flash | 16 MB. The boot image selects DIO at 80 MHz. |
| PSRAM | 8 MB OPI at 40 MHz. Two framebuffers use a reserved region of 518,400 bytes. |
| Application CPU clock | 240 MHz |
| Executor | Application and touch on core 0. Display refresh on core 1. Embassy through esp-rtos. |
| Application logging | USB Serial/JTAG only |
| Display | ED047TC1, 960 × 540, 4.7-inch grayscale e-paper |
| Radio | SX1262. Driver and protocol selection stay undecided. |

The vendor's [board definition](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/boards/T5-ePaper-S3.json)
specifies flash and PSRAM.
The [vendor README](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/README.md)
gives information about models and component variations.
Touch and GPS parts can differ between units.
Driver selection depends on identification of the actual parts.

The ESP32-S3 ROM cannot load a bootloader with a QIO header.
Use DIO for the boot image.
Espressif's [boot log reference][boot-header] explains this requirement in
"Software Bootloader Header Info".
The second-stage bootloader can select another flash mode after ROM startup.

[boot-header]: https://docs.espressif.com/projects/esptool/en/release-v4/esp32s3/advanced-topics/boot-mode-selection.html#software-bootloader-header-info

## Adapter topics

| Topic | Contents |
| --- | --- |
| [Display](lilygo-t5-s3-pro-display.md) | Native storage, scan output, waveform evidence, and limits |
| [Touch](lilygo-t5-s3-pro-touch.md) | Report protocol, interrupts, shared I2C, and Home-key design |
| [GNSS](lilygo-t5-s3-pro-gnss.md) | UART, baud selection, NMEA decoding, and observation validity |
| [RTC](lilygo-t5-s3-pro-rtc.md) | Calendar retention, validity, and GPS synchronization |
| [Power](lilygo-t5-s3-pro-power.md) | Battery measurements, charger status, faults, and verification limits |
| [Radio discovery](lilygo-t5-s3-pro-radio.md) | Passive scans, resource release, and diagnostic builds |

## Device inventory

The [device interface design](../device-interfaces.md) defines proposed shared
services and simulator scenarios.
Current firmware connects the clock, display, single-contact touch input, passive radio scans, GNSS observations, and power observations.
The [simulator guide](../simulator.md) describes the native backend.
Other services in this inventory stay design work.

| Hardware | Connection / expected variant | Shared representation |
| --- | --- | --- |
| E-paper panel and supply | ED047TC1 and TPS651851, I2C `0x68`, parallel pixel bus | Display completion, presentation validity, faults. The backend controls waveform history. |
| Touch controller | GT911 baseline, I2C `0x5D`. Vendor examples also use GT6972P. | Pointer input, separate touch-key reports, optional pen capabilities |
| Physical controls | S1–S4 and the touch-panel Home key where fitted | Software buttons and board lifecycle controls. The control table gives details. |
| Frontlight | PT4103B23F boost driver, enable/PWM on GPIO11 | Brightness request and applied state, independent of e-paper refresh |
| GNSS receiver | MIA-M10Q or L76K on UART, switched supply shared with LoRa | Location, fix status, independently valid UTC observations |
| LoRa radio | SX1262 module, shared SPI, dedicated CS/IRQ/reset/busy | Packet radio below the future mesh protocol |
| microSD socket | SPI, dedicated CS GPIO12, main 3.3 V supply | Removable files, media identity, mount state, errors, durability |
| External RTC | PCF8563TS, I2C `0x51`, IRQ GPIO2, J12 backup source | Wall-clock validity and wake alarms, separate from monotonic timers |
| Battery charger / system power path | BQ25896, I2C `0x6B`, QON control S4 | External power, charge state, bounded charge policy, battery disconnect request |
| Fuel gauge | BQ27220, I2C `0x55`, battery sense resistor | Battery measurements with timestamps, validity, and calibration status |
| I/O expander | PCA9535, I2C `0x20`, IRQ GPIO38 | Internal resource owner for buttons, auxiliary power, and display control |
| USB-C | VBUS power, ESP32-S3 D−/D+ on GPIO19/20 | Independent power connection and USB data or diagnostic session |
| Wi-Fi / Bluetooth LE | Built into ESP32-S3 | Passive discovery on request. No connection or pairing service. |
| Internal flash | 16 MB, also stores executable firmware | Durable settings and internal data. The backend controls partitions and updates. |
| PSRAM | 8 MB OPI, display storage enabled | Volatile memory, not persistent storage |
| Expansion connections | P3 exposes I2C/power. P5 exposes radio IRQ/reset/power nets. | Board profile extensions, not independent application devices |

Sources include [schematic][schematic] sheets 1–4, the
[vendor product table][product], and the [GT6972P example][pen-example].
The RTC is **PCF8563**, not PCF85063.
The alternative touch example does not identify the part or pen support in the
expected unit.

The reviewed schematic shows no accelerometer, gyroscope, magnetometer,
ambient-light sensor, speaker, or haptic actuator.
These are not established stock-board capabilities.
Automatic orientation depends on an added sensor or new evidence about the delivered
variant. Manual orientation operates without a sensor.
GNSS course over ground does not give device attitude.

J3 shows no GNSS PPS connection.
Do not reserve GPIO1 for PPS from an example definition.
That pin connects to LoRa reset.

## Contract mapping and backend ownership

The [portable service catalog](../device-interfaces.md#service-catalog) defines
observable behavior. This backend controls the implementation choices below.
Other backends can meet the same contracts with different parts or limits.

| Board fact | Backend responsibility | Portable consequence |
| --- | --- | --- |
| Native panel is 960 × 540. The demonstration uses a quarter-turn. | Supply geometry and mounting. Implement native display storage. | Layout receives a logical viewport. Dimensions do not define the architecture. |
| BOOT, function, and touch-controller Home reports | Supply local control identities, labels, event capabilities, and default bindings. | Shared policy uses declared controls without a universal `Boot`/`Home` enum. |
| Reset and charger QON operate outside the application | Model reset, power effects, and observable boot information. | No invented key callback or guaranteed graceful shutdown |
| LoRa and GNSS share the switched auxiliary rail | Keep the rail enabled while either adapter uses it. | Location shutdown must not interrupt accepted radio work. |
| SD and LoRa share SPI | Bound transactions. Control access with the applicable device settings. | File and radio requests report deadlines, progress, and backpressure without bus details. |
| PCA9535 controls display power, auxiliary power, and function input | Use one output-state owner and interrupt dispatcher. Report failures to affected services. | Shared resource failures appear through affected services. |
| Wake routing depends on pins and peripheral power | Check logical wake combinations against the wiring. | Accept a supported plan or reject it explicitly. |
| No established orientation or ambient-light sensor | Report automatic sources as unavailable. | Manual rotation and brightness operate without a sensor. Other profiles can supply automatic sources. |
| No wired SD card-detect or dedicated SD power switch | Detect media through probing and I/O errors. Separate unmount from power removal. | Presence detection can be delayed. Unmount does not guarantee power savings. |

The installed GNSS receiver still requires a physical identification check.
The [vendor example][gnss] tries L76K at 9600 baud and MIA-M10Q at 38400 baud.
Parsing and baud selection stay below `DEV-LOCATION`.
GNSS-derived UTC can also supply `DEV-TIME`.
UART receipt on this wiring does not provide PPS precision.

The [fuel-gauge example][gauge] identifies available measurements.
It does not establish calibration or accuracy for the delivered cell.

ESP light sleep is a candidate for `DEV-POWER` suspend with resume.
Deep sleep restarts the application.
BQ25896 battery shipping mode is a conditional power-off mechanism.
USB can keep the system supplied.

Shared policy requests outcomes through capabilities.
It does not select charger registers, GPIO polarities, or processor sleep enums.
The backend resource coordinator controls shared rail leases and shutdown order.

## All physical controls

Schematic designators identify controls independently of enclosure position or
application binding. Do not rename controls when the screen rotates.
Confirm the printed labels and accessible positions on the delivered enclosure.

| Control | Electrical path | Application / simulator meaning |
| --- | --- | --- |
| **S1 — reset / RST** | Pulls ESP32 `EN` low, sheet 2 | Immediate MCU reset outside application events. Simulation must model restart and interrupted work, not a button callback. |
| **S2 — BOOT** | Active-low GPIO0, sheet 2 | Runtime `Boot` input. A hold during reset selects the ROM download path. |
| **S3 — function** | Active-low `BUTTON`, PCA9535 `IO1_2` (flat index 10), sheets 2–3 | Runtime `Function` input. Expander index 10 is different from ESP GPIO10, the LoRa IRQ. |
| **S4 — QON / power recovery** | BQ25896 `QON` to ground, sheet 1 | Charger-controlled battery wake or reset. No direct MCU key input or ordinary software power toggle. |
| **Capacitive Home key** | Touch-controller key report, shared touch IRQ GPIO3 | Logical `Home` activation independent of screen coordinates. Presence and edge reports depend on capabilities. |
| **Optional pen controls** | GT6972P report with the applicable controller and pen | Pen buttons 1/2 and eraser state. Separate from enclosure keys. Unavailable in the baseline until detection. |

The [factory touch initialization][factory-touch] installs a Home callback.
The [GT911 driver][gt911] calls it from the controller key-report flag.
This callback alone does not establish reliable press and release timing.
GT911 documents press and release status.
The activation-only callback is a driver limit, not a proven controller limit.
Edge capability must pass checks on the fitted panel.

The demonstration's screen button is different from the physical Home key.

QON exits battery shipping mode.
A long hold can reset the battery power path, subject to charger configuration
and external power. TI specifies a typical 15-second reset hold.
This duration is not an application gesture threshold.

The vendor shutdown example distinguishes battery-only operation from USB power.
With USB attached, battery disconnection does not switch the system off.
[BQ25896 section 9.2.10][charger] and the [shutdown example][shutdown] give information about
these conditions.

The [touch design](lilygo-t5-s3-pro-touch.md#gt911-home-key-configuration-and-lower-bezel) records the Home-key investigation and remaining checks.

## Shared buses, power domains, and wake routing

| Resource | Consumers / wiring | Design constraint |
| --- | --- | --- |
| Main I2C, SDA39/SCL40 | Touch, RTC, expander, charger, fuel gauge, display supply | One serialized bus owner. Failure and recovery can affect multiple services. |
| Shared SPI, SCK14/MOSI13/MISO21 | SD CS12 and LoRa CS46 | Device-specific configuration and exclusive transactions. Bounded SD work must permit radio progress. |
| Main `VDD3V3` | MCU, SD, expander, touch, logic | No separate SD power switch appears in the schematic. Sleep and power removal are different. |
| Auxiliary `VCC3V3` | LoRa and GNSS, controlled by PCA9535 `IO0_0` / `LORA_EN` | Both clients share one rail. Neither driver can remove power while the other uses it. |
| Display high-voltage rails | TPS651851, with control and status through PCA9535 | The display driver controls sequences, power-good checks, faults, and safe shutdown. |
| Frontlight supply | Boost converter from `VSYS`, GPIO11 enable/PWM | Can illuminate a retained image with panel scan power off. |
| RTC backup `VRTC` | PCF8563 and J12 / MS412FE source | Retention depends on a fitted, charged backup source. Checks must include system power loss. |

[Schematic][schematic] sheets 1–4 and the [vendor pin map][pinmap] establish
these connections. These sheets show no confirmed MCU input route for charger
`INT` or gauge `GPOUT`. Initial battery observations therefore use polling.
They cannot assume hardware notifications.

Sheet 2 shows no MCU connection for the SD socket's `CD#`.
Insertion and removal detection use probing and I/O errors.
An enabled rail or asserted interrupt does not prove device readiness.

| Event | Wake representation |
| --- | --- |
| BOOT GPIO0 | Candidate RTC-GPIO wake source, with separate reset and boot-strapping behavior |
| Function key through PCA IRQ38 | Ordinary GPIO wake in light sleep depends on the bus and expander. GPIO38 is outside the RTC-GPIO wake set. |
| Touch/Home through IRQ3 | Candidate RTC-GPIO wake if the powered touch controller can assert its interrupt |
| RTC alarm through IRQ2 | Candidate RTC-GPIO wake after alarm configuration and pending-flag handling |
| LoRa IRQ10 | Candidate RTC-GPIO wake with the radio and auxiliary rail in a suitable receive mode |
| GNSS UART RX44 | Separate UART light-sleep wake capability. First-message retention and deep-sleep wake are not guaranteed. |
| Reset, QON, USB power restoration | Board reset or power transition, not input to a resumed application |

ESP32-S3 external RTC-GPIO wake uses pins 0–21.
Ordinary GPIO wake in light sleep has different constraints.
Thus, the function key cannot replace BOOT as a deep-sleep wake source.
The sleep planner must reject unsupported requests.
It must not silently select a mode that cannot supply the requested wake event.

These candidates come from wiring evidence.
They are not verified wake configurations.
[Espressif's sleep-mode reference][sleep] gives information about silicon restrictions.
Use of this reference does not select an ESP-IDF application runtime.

## Pin-definition coverage

The comparison of [H752-01 pin definitions][pin-definitions-live] with the pinned
file occurred on 2026-10-12.
The branch head matched the reviewed commit.
The files were byte-identical.
Each defined signal family has an owner:

| Pin-definition group | Inventory / service owner |
| --- | --- |
| GPS RX/TX and UART aliases | GNSS receiver / DEV-LOCATION |
| Main I2C and touch/RTC aliases | Shared bus / backend resource coordinator. Aliases do not add buses. |
| Main SPI and SD/LoRa aliases | Shared bus / backend resource coordinator for DEV-FILES and DEV-RADIO |
| Touch reset and interrupt | Touch controller / DEV-INPUT |
| RTC interrupt | External RTC / DEV-TIME and DEV-POWER wake planning |
| LoRa CS, IRQ, reset, busy | SX1262 / DEV-RADIO |
| SD CS | Removable storage / DEV-FILES |
| Frontlight enable | DEV-LIGHT |
| BOOT and expander interrupt | Physical input / DEV-INPUT, with shared interrupt dispatch in the backend resource coordinator |
| E-paper data, clock, latch, and start signals | Native display backend / DEV-DISPLAY |
| Expander port 1 output-enable/mode and TPS control/status | Display backend through the resource coordinator, including VCOM, power-good, and faults |
| Expander port 1 button | S3 Function / DEV-INPUT |
| Unnamed expander port 0 definitions | Bit 0: shared LoRa/GNSS rail. Bits 1–7: unconnected in the reviewed schematic. |

The macro list omits reset, QON, the Home key, charger, gauge, USB wiring, and
backup-power details. The schematic and examples supply those inventory details.
Unnamed definitions do not prove more peripherals or usable board
connections.

## Peripheral pins

The following ESP32 GPIO numbers come from the
[factory pin definitions](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/utilities.h).

| Interface | Pins |
| --- | --- |
| I2C | SDA 39, SCL 40 |
| E-paper data D0–D7 | 5, 6, 7, 15, 16, 17, 18, 8 |
| E-paper clocks and controls | CKH 4, CKV 48, STH 41, STV 45, LEH 42 |
| Shared SPI | SCK 14, MOSI 13, MISO 21 |
| SX1262 | CS 46, IRQ 10, reset 1, busy 47 |
| microSD | CS 12 |
| Touch | interrupt 3, reset 9 |
| GPS UART | MCU RX 44, MCU TX 43 |
| Frontlight enable | 11 |
| RTC interrupt | 2 |
| PCA9535 interrupt | 38 |
| Boot button | 0 |
| Function button | PCA9535 `IO1_2` / index 10, shared expander IRQ38 |
| Reset button | ESP32 `EN`, not a GPIO |
| QON button | BQ25896 `QON`, not a GPIO |

GPIO 43/44 are also the usual UART0 pins.
`esp-println` disables automatic transport selection.
Application and panic logs therefore cannot use the GPS UART as an alternative.
ROM and bootloader logs are separate from this application setting.

The display's parallel signals and power sequences must operate together. The vendor's [complete pin table](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pin_define.md)
gives the connections.
Multiple display control signals use the PCA9535 expander and TPS65185 supply.
The BQ25896 charger and BQ27220 gauge must also have deliberate configuration.
Unconfigured devices do not establish managed battery operation.

## Factory backup

1. Confirm the board revision and USB device identity.
2. Configure serial access in the local devcontainer.
3. Run `espflash board-info` with the explicit port and `--after no-reset`.
4. Check the reported chip, flash size, and security state.
5. If the chip is not ESP32-S3 or the flash size differs from 16 MB, stop.
6. If security settings restrict flash access, stop.
7. Create a new backup directory under `backups/` with access limited to the owner.
8. Save the device information and espflash version in that directory.
9. Read all `0x1000000` bytes from address `0x0` with `espflash read-flash`.
10. Read the same region into a second file.
11. Compare both files with `cmp`.
12. Check that each file contains 16,777,216 bytes.
13. Create SHA-256 checksums for both files.

Use the explicit port and `--after no-reset` for both reads.
This option keeps the device in the bootloader between reads.
The read commands do not erase or write flash memory.

The backup contains the bootloader, partition table, applications, and stored data.
Git ignores `backups/`. This directory is separate from build output.
Backup completion requires two identical reads of the full flash memory.

## First hardware checks

1. Confirm the board revision.
2. Check USB and serial passthrough into the devcontainer.
3. When flashing is requested, save the factory firmware before replacement.
4. Check the boot banner and heartbeat through USB.
5. Check recovery after reset and USB disconnection and reconnection.
6. Establish I2C identification, battery measurements, and controlled peripheral power before display, touch, or radio operation.
7. Identify all physical controls, the touch variant, Home key, GNSS variant, and backup battery.
8. Check each intended sleep mode's wake sources separately.
9. Check SD removal and concurrent SD and radio activity.
10. Check auxiliary-rail sharing.
11. Compare shutdown with USB power and battery-only power.
12. Check recovery after reset during I/O.

Current firmware initializes display storage, display output, GT911 input, requested radio scans, and continuous GNSS input.
Other peripherals stay uninitialized.
This state does not guarantee that those peripherals are off.
Timer waits yield to the scheduler.
Automatic light sleep and deep sleep stay unconfigured.
Battery-life estimates must use measurements on the physical board.

MeshCore integration, persistent storage, and OTA
partitions and updates stay separate work.

## Backend conformance scenarios

These are planned additions to the
[portable acceptance scenarios](../platform/service-validation.md).
They are not implemented simulator controls or verified hardware behavior.
A simulator preset for this board must model these constraints.
Synthetic profiles with different capabilities can also verify portable
application behavior.

| Board stimulus | Expected mapping to shared contracts |
| --- | --- |
| S1 reset during SD write or display refresh | New application epoch. Interrupted work can be lost. Panel content can stay, with unknown driver history. |
| S2 BOOT bounce, held at startup, or held during reset | Shared gestures handle runtime edges. Startup synchronization prevents incorrect clicks. A hold during reset can select ROM download mode without an application. |
| S3 function and TPS fault assert shared PCA IRQ38 | Correct dispatch without incorrect button events, lost faults, or overwritten output state |
| GT911 Home and optional GT6972P pen | Only verified event kinds and tool capabilities. No invented bezel coordinates. |
| S4 QON short or long hold, with battery-only or USB power | Charger-specific power and reset behavior. No generic application power-key callback. |
| GNSS search, lost fix, or either receiver variant | Same validity and age contract. Independent position and time validity. |
| GNSS stops while LoRa receives | Auxiliary rail stays enabled for the radio. |
| SD insertion, removal, or replacement during I/O | Declared probing and error-detection delay. Rejection of stale handles and completions. |
| SD traffic competes with LoRa | Bounded SPI transactions and observable radio backpressure |
| RTC backup loss or UTC correction | Invalid civil time without changed monotonic deadlines |
| USB connection during battery shutdown preparation | Another check of the plan. Command acceptance does not confirm power removal. |
| Function key requested as the only deep-sleep wake source | Rejection of the unsupported plan. An alternative must have an explicit policy choice. |
| Frontlight changes with EPD scan power off | Light can change independently of refresh. |
| Shared I2C failure and recovery | Cancellation of uncertain input. Stale or unavailable dependent data. Restored resource state. |
| Automatic rotation requested on the baseline board | Unavailable automatic source. Manual orientation stays possible in the design. |

[schematic]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/T5%20E-paper%20S3%20Pro%20V1.0%2024-12-24.pdf
[product]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/README.md
[pinmap]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pinmap.md
[pin-definitions-live]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/H752-01/docs/pin_define.md
[factory-touch]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/main.cpp#L328-L351
[gt911]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L203-L213
[pen-example]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/touch_gt6972p/README.md
[charger]: https://www.ti.com/lit/ds/symlink/bq25896.pdf#page=26
[shutdown]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq25896/main/main.ino#L55-L61
[sleep]: https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/sleep_modes.html
[gnss]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino#L80-L96
[gauge]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq27220/main/main.ino
