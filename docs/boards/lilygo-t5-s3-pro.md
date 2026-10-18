# LILYGO T5 S3 E-Paper Pro backend profile

This profile maps one hardware target to the portable
[application architecture](../architecture.md) and
[service contracts](../device-interfaces.md).
Its chips, controls, dimensions, and wake restrictions apply to this backend.
Other targets can have different characteristics.

The initial target is the **H752-01 full Pro with LoRa**.
The older H752 and Lite models are different targets.
This profile uses LILYGO sources at commit
[`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
The inventory review used the schematic and examples on **2026-10-12**.

When the device arrives, confirm its revision and fitted components.
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

### USB bootstrap check, 2026-10-17

The QIO image caused repeated `ets_loader.c 78` errors before application startup.
The factory backup had a DIO boot header.
The DIO image started the same application ELF successfully.

Espflash 4.6.0 verified the flash write on the connected ESP32-S3, revision v0.2.
USB output showed the `paper-mesh-a 0.1.0` boot message and an 80 MHz CPU clock.
The application sent heartbeat messages at 30 and 60 seconds.
These checks do not verify display, touch, power management, or radio operation.

### Display and touch implementation

The later display build uses a 240 MHz CPU clock and initializes OPI PSRAM.
The [display evidence](lilygo-t5-s3-pro-display.md#current-rust-implementation) records its implementation and physical checks.
The user confirmed that the shared interface appeared clearly on the panel.

[`crates/gt911`](../../crates/gt911/src/lib.rs) implements the GT911 report protocol.
[`firmware/src/touch.rs`](../../firmware/src/touch.rs) adapts reports to the shared touch contract.
The driver probes both documented addresses and checks the product identity.
If neither probe succeeds, it applies the documented reset sequence and selects address `0x5D`.
It records the existing configuration through USB without configuration writes.

The connected controller reported GT911 firmware `0x1060`, vendor ID 2, and 540 × 960 coordinates at address `0x5D`.
The initial probes received no address acknowledgement.
The reset sequence made the controller available to the subsequent boot.
The saved USB log contains all 186 bytes from `0x8047` through `0x8100`.

USB output recorded complete taps at logical positions `(345, 482)` and `(359, 480)`.
The shared counter advanced to 1 and 2, with a completed refresh after each change.
The user confirmed that the physical screen showed the increased count.
These checks establish basic single-contact operation, not complete edge calibration or gesture coverage.

Reports must have the ready flag before the driver reads coordinates.
The driver acknowledges each report after the required reads succeed.
It emits one press and one release for each single-contact gesture.
Multiple contacts, invalid coordinates, I2C failures, or event queue overflow cancel the gesture.
The Home key and pen input stay unimplemented.

[Goodix Programming Guide Rev.10][gt911-guide], sections 3.3, 4.2, and 5, defines identity, reset timing, and report handling.
The adapter converts the fitted portrait coordinates through the shared native-to-portrait transform.

### Rapid tap investigation, 2026-10-17

The user reported missed taps during rapid input after the partial update check.
The rebase and flash onto `main` commit `ac2a3af9` preceded this investigation.
The rebased build passed all 38 native tests and the complete `just ci` checks.
USB confirmed startup, touch detection, display completion, and heartbeats at 30 and 60 seconds without faults.

The previous touch adapter polled every 20 ms while idle, but only after each complete display scan during refresh.
Each partial scan sends 541 rows, including the final neutral row.
Each row requires a 100 µs high pulse and a 5 µs low pulse.
These pulses alone require 56.805 ms between touch polls.
Encoding, transfers, rendering, and supply waits add further delays.

The saved controller configuration sets `Refresh_Rate` at `0x8056` to `0x0F`, which selects a 20 ms report period.
[Goodix Programming Guide Rev.10][gt911-guide], sections 3.2 and 5, defines this period and the acknowledgment requirement.
The controller retains an unread report instead of replacing it with new coordinates.
Thus, rapid release and press changes can occur before the host acknowledges the previous report.
This timing gap can explain missed taps. The software queue cannot recover reports that never reached the host.

### Interrupt and multicore implementation

The display worker runs on core 1 with an 8 KiB stack.
The application transfers exclusive panel ownership through a channel before each refresh.
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
The touch queue holds 32 events. Overflow cancels the gesture and increments an error counter.

USB reports the core assignment, interrupt times, service duration, bus wait, and events received during refresh.
Service duration starts at GPIO handler entry and ends after the report read and acknowledgment attempt.
It excludes interrupt latency before handler entry.
The native burst test verifies all 40 taps across multiple pending refreshes and compares the final image pixel by pixel.

The implementation follows esp-rtos 0.4.0
[`start_second_core`](https://github.com/esp-rs/esp-hal/blob/63e97be553f435f29171f7ae5f37a41ad737a75a/esp-rtos/src/lib.rs#L440-L458)
and [`InterruptExecutor`](https://github.com/esp-rs/esp-hal/blob/63e97be553f435f29171f7ae5f37a41ad737a75a/esp-rtos/src/embassy/mod.rs#L308-L419).
The first API pins the display thread to core 1.
The second API starts touch service on the core that installs its software interrupt.

### Rapid tap check, 2026-10-17

The complete `just ci` checks passed, including 39 native tests.
Espflash verified the firmware write to the same ESP32-S3 device.
USB confirmed touch service on core 0 and display refresh on core 1.
Startup completed a full refresh in 1,598 ms. Heartbeats continued at 30 and 60 seconds.
The GT911 responded at `0x14` on this boot, with the same 186 configuration bytes as earlier touch checks.

The user supplied exactly 20 rapid taps without waits for display completion.
USB recorded 20 presses, 20 releases, and consecutive counts from 1 through 20.
The burst lasted 4.231 seconds, with press intervals of 187–251 ms and contact durations of 62–84 ms.
The average press rate was 4.557 taps per second.
Thirty-six of the 40 contact events received service during display refresh.
The user confirmed a final count of 20 with clear digits.

The maximum measured service duration was 2.224 ms. The maximum bus wait was 0.701 ms.
The log contained no I2C errors, queue overflows, display faults, or additional boots.
Partial refreshes took 879–903 ms at 24 °C.
Counts that arrived during refresh combined into later frames without losing gestures.
Repeated low-level interrupts included 1 ms retries while the pin remained asserted.
The idle interrupt counter stayed at 1,677 at the 60-second and 90-second heartbeats.

This check verifies the reported rapid tap case at the measured rate.
It does not establish a maximum input rate or latency before GPIO handler entry.

## Device inventory

The [device interface design](../device-interfaces.md) defines proposed shared
services and simulator scenarios.
Current hardware firmware connects the clock, display, single-contact touch input, and passive radio scans.
The simulator implements display and single-contact touch services.
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

The GNSS adapter must identify the fitted receiver.
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

### GT911 Home key configuration and lower bezel

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
These findings do not implement a driver or simulator model.

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

## Radio discovery

`firmware/src/discovery/` implements the shared `Discovery` interface.
One Embassy worker owns `WIFI` and `BT` on core 0.
It initializes only the radio for the active request.
It destroys that controller before it delivers results or starts another scan.
Cancellation invalidates observations immediately and wakes the worker for resource release.
The request deadline includes initialization, the scan, and resource release.

The firmware reserves 128 KiB of internal RAM for radio drivers and RTOS tasks.
Radio futures also use this heap to limit temporary stack storage.
Application results and worker results use separate static buffers, each with 24 entries.
The shared application does not allocate heap memory.
The CPU operates at 240 MHz for display scans.
Display storage uses PSRAM. Core 1 executes display refreshes.

| Radio | Scan configuration | Result conversion |
| --- | --- | --- |
| Wi-Fi | Passive scan, 120 ms per channel, hidden SSIDs included, default driver country configuration | BSSID, SSID, signed RSSI. The adapter requests up to 25 records and retains 24. An extra record sets the result-limit indication. |
| Bluetooth LE | Passive legacy scan, 1M PHY, 100 ms interval and window, four-second observation period, duplicate filtering disabled | Public or random address, advertised local name, signed RSSI. RSSI 127 means unavailable. Addresses use display byte order. |

The Wi-Fi driver supplies its records in signal order.
The shared result buffer applies the final order and address tie-break.
The Wi-Fi API exposes only the valid UTF-8 prefix of an SSID.
The adapter replaces unavailable suffix bytes with question marks.
An empty SSID stays a hidden network.

Bluetooth reports update RSSI for the same address and address type.
An omitted name preserves a previous name from the same scan.
A complete local name takes precedence over a shortened name within one advertisement.
Malformed advertising data produces an unnamed observation.
Malformed HCI reports produce a scan failure and discard partial results.
The adapter does not resolve private addresses into stable identities.

Passive scans do not send probe requests or scan requests.
Thus, a device name that appears only in a scan response can stay unavailable.
The Bluetooth adapter does not request extended or coded-PHY advertisements.
Wi-Fi discovery covers the ESP32-S3's 2.4 GHz radio.
These scans do not establish a connection, pair a device, or transmit application data.
LoRa stays uninitialized.

The implementation pins `esp-radio` to `1.0.0-beta.1` and uses `bt-hci` 0.9.
The lockfile fixes the resolved dependencies.
These upstream sources define the adapter assumptions:

| Source | Applicable sections |
| --- | --- |
| [esp-radio Wi-Fi source][radio-wifi] | `WifiController::new`, `scan_async`, `WifiRefGuard::drop`, `wifi_deinit`, and `Ssid::as_str` |
| [esp-radio scan source][radio-scan] | `ScanConfig`, `ScanTypeConfig::Passive`, and `ScanResults` |
| [esp-radio Bluetooth source][radio-ble] | `BleConnector::new`, `Drop`, and the HCI transport implementation |
| [esp-radio runtime source][radio-runtime] | Allocator requirement, `init`, and minimum CPU frequency |
| [bt-hci controller source][hci-controller] | Concurrent command acknowledgment and event reception in `ExternalController` |
| [bt-hci command source][hci-commands] | `LeSetScanParams`, `LeSetScanEnable`, and Bluetooth specification references |
| [Espressif Wi-Fi reference][wifi-records] | `esp_wifi_scan_get_ap_records` signal order and scan record lifecycle |

The pinned Wi-Fi wrapper does not expose `ScanDone.status` to its caller.
The adapter reports errors that the wrapper returns.
It cannot distinguish every failure inside the driver from an empty result.
Controller destruction also handles shutdown errors inside the driver.
Physical tests must therefore check repeated scans, cancellation, and radio changes for resource leaks or continued radio activity.
The compile checks and native tests do not establish physical operation or heap capacity under radio traffic.

### Diagnostic build

The default firmware waits for shared application requests.
The display and touch adapters connect the shared debugger to the physical screen.
The optional `radio-scan-demo` feature performs one Wi-Fi scan and one Bluetooth LE scan at boot.
It prints the result order, addresses, names, RSSI, errors, and result-limit indication through USB Serial/JTAG.

To build this diagnostic firmware without device access, run:

```sh
devcontainer exec --workspace-folder . bash -lc 'cd firmware && cargo build --locked --release --features radio-scan-demo'
```

This command writes the ELF under `firmware/target/xtensa-esp32s3-none-elf/release/`.
`just ci` checks this feature, then produces the default firmware image.
Neither command flashes a device.

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

Current firmware initializes display storage, display output, GT911 input, and requested radio scans.
Other peripherals stay uninitialized.
This state does not guarantee that those peripherals are off.
Timer waits yield to the scheduler.
Automatic light sleep and deep sleep stay unconfigured.
Battery-life estimates must use measurements on the physical board.

MeshCore integration, persistent storage, and OTA
partitions and updates stay separate work.

## Backend conformance scenarios

These are planned additions to the
[portable acceptance scenarios](../device-interfaces.md#simulator-acceptance-scenarios).
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
[gt911-init]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L546-L645
[gt911-irq]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L275-L292
[gt911-config]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L425-L485
[touch-coordinate-helper]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvInterface.cpp#L77-L99
[gt911-guide]: https://www.lcd-module.de/fileadmin/eng/pdf/zubehoer/GT911_Programming_Guide_Rev.10.pdf
[gt911-datasheet]: https://www.olimex.com/Products/OLinuXino/LCD/_resources/GT911_Datasheet.pdf
[pen-example]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/touch_gt6972p/README.md
[charger]: https://www.ti.com/lit/ds/symlink/bq25896.pdf#page=26
[shutdown]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq25896/main/main.ino#L55-L61
[sleep]: https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/sleep_modes.html
[gnss]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino#L80-L96
[gauge]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq27220/main/main.ino
[radio-wifi]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/wifi/mod.rs
[radio-scan]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/wifi/scan.rs
[radio-ble]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/ble/controller/mod.rs
[radio-runtime]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/lib.rs
[hci-controller]: https://github.com/embassy-rs/bt-hci/blob/d0f7558ff205313c2c473046adec0ffdc00d919b/src/controller.rs
[hci-commands]: https://github.com/embassy-rs/bt-hci/blob/d0f7558ff205313c2c473046adec0ffdc00d919b/src/cmd/le.rs
[wifi-records]: https://docs.espressif.com/projects/esp-idf/en/v5.2.6/esp32s3/api-reference/network/esp_wifi.html#_CPPv428esp_wifi_scan_get_ap_recordsP8uint16_tP16wifi_ap_record_t
