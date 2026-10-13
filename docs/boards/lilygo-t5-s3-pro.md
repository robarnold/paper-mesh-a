# LILYGO T5 S3 E-Paper Pro backend profile

This profile maps one supported hardware target to the portable
[application architecture](../architecture.md) and
[service contracts](../device-interfaces.md). Its chips, controls, dimensions,
and wake restrictions are backend facts, not requirements for other targets.

The initial target is the **H752-01 full Pro with LoRa**, not the older H752 or
the Lite model. This profile comes from LILYGO's repository at commit
[`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
Confirm the revision and fitted components when the device arrives.
Inventory reviewed on **2026-10-12** against the pinned schematic and examples.
The [hardware reference index](../vendor/lilygo/t5-s3-pro/README.md) links to the
vendor's schematics, datasheets, pin maps, and mechanical drawings.
Panel scan geometry, waveform/driver limitations, and the evidence needed for
application performance budgets are documented in
[display backend evidence](lilygo-t5-s3-pro-display.md).

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

## Device inventory

The [device interface design](../device-interfaces.md) defines the proposed shared
services and simulator scenarios for this inventory. Only the clock/heartbeat
is connected on hardware today. Display and single-contact touch have simulator
implementations; the other services below are design work, not working drivers.

| Hardware | Connection / expected variant | Shared representation |
| --- | --- | --- |
| E-paper panel and supply | ED047TC1 plus TPS651851, I2C `0x68`; parallel pixel bus | Display completion, presentation validity, and faults; backend owns waveform history |
| Touch controller | GT911 baseline, I2C `0x5D`; vendor also provides GT6972P examples | Pointer/contact input, separately reported touch keys, optional pen capabilities |
| Physical controls | S1–S4 plus the touch-panel Home key where fitted | Software buttons and board lifecycle controls; see the full control table below |
| Frontlight | PT4103B23F boost driver, enable/PWM on GPIO11 | Brightness request and applied state, independent of e-paper refresh |
| GNSS receiver | MIA-M10Q or L76K on UART, shared switched supply with LoRa | Location/fix status and independently valid UTC observations |
| LoRa radio | SX1262 module, shared SPI, dedicated CS/IRQ/reset/busy | Packet radio beneath the eventual mesh protocol |
| microSD socket | SPI, dedicated CS GPIO12, main 3.3 V supply | Removable file storage, media identity, mount/error/durability state |
| External RTC | PCF8563TS, I2C `0x51`, IRQ GPIO2, J12 backup source | Wall-clock validity and wake alarms; distinct from monotonic timers |
| Battery charger / system power path | BQ25896, I2C `0x6B`, QON control S4 | External-power/charging state, bounded charging policy, battery disconnect request |
| Fuel gauge | BQ27220, I2C `0x55`, battery sense resistor | Timestamped battery measurements with validity and calibration status |
| I/O expander | PCA9535, I2C `0x20`, IRQ GPIO38 | Internal board resource owner, shared by buttons, auxiliary power, and display control |
| USB-C | VBUS power; ESP32-S3 D−/D+ on GPIO19/20 | Separate power attachment and USB data/diagnostic session states |
| Wi-Fi / Bluetooth LE | Built into ESP32-S3 | Optional connectivity services; neither is initialized by this firmware |
| Internal flash | 16 MB, also contains executable firmware | Durable settings / internal data; partition and update ownership below the app |
| PSRAM | 8 MB OPI, currently uninitialized | Volatile memory resource, never persistent storage |
| Expansion connections | P3 exposes I2C/power; P5 exposes radio IRQ/reset/power nets | Board-profile extensions; not independent application devices |

Sources: [schematic][schematic], sheets 1–4; [vendor product table][product];
[GT6972P example][pen-example]. The RTC is **PCF8563**, not PCF85063.
The example for a different touch controller does not establish which part or
pen support our arriving unit has.

No accelerometer, gyroscope, magnetometer, ambient-light sensor, speaker, or
haptic actuator appears in the reviewed schematic. Do not advertise those as
stock-board capabilities. In particular, automatic orientation needs an added
sensor or new evidence about the shipped variant; manual orientation can be
implemented without a sensor. GNSS course over ground is not device attitude.
No GNSS PPS connection is shown on J3; do not reserve GPIO1 for PPS based on an
example's define, since that pin is wired to LoRa reset.

## Contract mapping and backend ownership

The [portable service catalog](../device-interfaces.md#service-catalog) defines
observable behavior. This backend owns the following implementation choices;
other backends can satisfy the same contracts with different parts or limits.

| Board fact | Backend responsibility | Portable consequence |
| --- | --- | --- |
| Native panel is 960 × 540; current demo uses a quarter-turn | Supply geometry and mounting; implement native display storage | Layout receives the logical viewport; dimensions do not define the architecture |
| BOOT, function, and touch-controller Home reports | Supply profile-local control identities, labels, event capabilities, and default bindings | Shared input policy handles declared controls without a universal `Boot`/`Home` enum |
| Reset and charger QON act outside the app | Model actual reset/power effects and observable boot information | No invented software key callback or guaranteed graceful shutdown |
| LoRa and GNSS share the switched auxiliary rail | Retain the rail while either adapter needs it | Stopping location acquisition must not break accepted radio work |
| SD and LoRa share SPI | Bound transactions and arbitrate per-device settings | File/radio requests expose deadlines, progress, and backpressure without exposing the bus |
| PCA9535 controls display power, auxiliary power, and the function input | One output-state owner and interrupt dispatcher; notify all affected services on failure | Shared resource failure is reported through affected services |
| Wake routing differs by pin and peripheral power | Validate logical wake combinations against the wiring | Accept a supported lifecycle plan or reject it explicitly |
| No evidenced orientation/ambient-light sensor | Advertise automatic sources as unavailable in this profile | Manual rotation and brightness policy needs neither sensor; other profiles can supply automatic sources |
| No wired SD card-detect or dedicated SD power switch | Probe and handle I/O errors; distinguish unmount from electrical power removal | Media presence can be delayed; unmount does not promise power savings |

The GNSS adapter must probe the fitted receiver: the [vendor example][gnss] tries
L76K at 9600 baud and MIA-M10Q at 38400. Parsing and baud selection remain below
`DEV-LOCATION`; GNSS-derived UTC can also feed `DEV-TIME`. UART receipt is not
PPS-grade timing on this wiring. The [fuel-gauge example][gauge] identifies
available measurements, not their calibration or accuracy on the delivered cell.

For `DEV-POWER`, ESP light sleep is a candidate implementation of suspend with
resume; deep sleep restarts the application. BQ25896 battery shipping mode is a
conditional power-off mechanism, and USB may keep the system supplied. Shared
policy requests those outcomes through capabilities; it does not select charger
registers, GPIO polarities, or a processor sleep enum. Shared rail leases and
quiesce ordering are owned by this backend's resource coordinator.

## All physical controls

Schematic designators identify controls independently of enclosure position or
application binding. Do not rename them when the screen rotates. Confirm the
printed labels and accessible positions against the delivered enclosure.

| Control | Electrical path | Application / simulator meaning |
| --- | --- | --- |
| **S1 — reset / RST** | Pulls ESP32 `EN` low, sheet 2 | Immediate MCU reset, outside app event handling; simulate restart and interrupted work, not a button callback |
| **S2 — BOOT** | Active-low GPIO0, sheet 2 | Runtime `Boot` button when configured as input; holding it during reset selects the ROM download path |
| **S3 — function** | Active-low `BUTTON`, PCA9535 `IO1_2` (flat index 10), sheets 2–3 | Runtime `Function` button; do not confuse expander index 10 with ESP GPIO10, which is LoRa IRQ |
| **S4 — QON / power recovery** | BQ25896 `QON` to ground, sheet 1 | Charger-managed battery-path wake/reset; no direct MCU key input, and not a normal software power toggle |
| **Capacitive Home key** | Touch-controller key report, shared touch IRQ GPIO3 | Separate logical `Home` activation, independent of screen coordinates; presence and edge reporting are capability-dependent |
| **Optional pen controls** | GT6972P report when that controller and a compatible pen are present | Pen button 1/2 and eraser state, distinct from enclosure keys; absent in the baseline profile until detected |

The [factory touch initialization][factory-touch] installs a Home callback.
The [GT911 driver][gt911] invokes it from the controller key-report flag; that
callback alone does not establish reliable press/release timing. GT911 itself
documents press/release status; an activation-only callback is a driver
limitation, not proof that the controller cannot report releases. Validate that
path on the fitted panel before advertising edge events.
An on-screen button drawn by our demo is not this physical Home key.

QON exits battery shipping mode and can cause a battery-path system reset on a
long hold, subject to charger configuration and external power. TI specifies a
typical 15-second reset hold, not an application gesture threshold. The vendor's
shutdown example explicitly distinguishes battery-only operation from USB
power: cutting the battery path while USB is attached does not turn the system
off. See [BQ25896 section 9.2.10][charger] and the [shutdown example][shutdown].

### GT911 Home key configuration and lower bezel

Reviewed on **2026-10-13**. The controller has configurable touch-key recognition,
but extra usable bezel area is **not established** for this panel.

[Goodix's programming guide, Rev.10][gt911-guide], section 3.2, describes:

| Registers | Meaning |
| --- | --- |
| `0x804E` bit 0 | Touch-key enable |
| `0x8093`–`0x8096` | Four key positions; zero disables a key; the guide identifies all-multiple-of-eight values with independent-key designs |
| `0x8097` | Lower nibble sets a one-sided active extent (0–15); upper nibble controls held-touch baseline updating |
| `0x8098`–`0x809B` | Key press/release thresholds and sensitivity |
| `0x805B`–`0x805C` | Cropping at the four touchscreen borders when the sensor grid extends beyond the visible display |

See guide pages 5–6, 9, and 12–13. Key-area values are not documented as pixel or
millimetre dimensions. Section 3.3, page 17, defines `HaveKey` as pressed/released
and requires a valid, ready report before interpreting it.

[Goodix's datasheet][gt911-datasheet], section 5.4 (printed page 8, PDF page 9),
describes up to four keys using a transmit channel separate from the main touch
matrix and shared receive channels, including a separate FPC electrode design.
This is not evidence of an arbitrary X/Y rectangle over the entire lower bezel.
Configuration cannot create electrodes where the panel has none. A larger key
response area would also not automatically provide useful X/Y tracking there.

The pinned [LILYGO factory initialization][factory-touch] installs the callback
and selects low-level interrupt reporting. The [driver initialization][gt911-init]
reads the existing controller settings; [interrupt setup][gt911-irq] changes the
interrupt bits and refreshes the configuration checksum. It does not install a
board-specific key-position/area table. The reviewed factory and touch examples
therefore do **not** tell us the shipped values of those registers. The
[`setMaxCoordinates` helper][touch-coordinate-helper] used in newer examples
only sets host-side mirroring bounds; it does not expand the sensor area.

The reviewed board schematic exposes the touch module's host connection, not its
electrode layout. No fitted-panel electrode drawing or configuration dump was
identified in these references. There are two distinct possibilities to test:
adjustable recognition on existing key electrodes, and additional main-matrix
area currently cropped at a border. Neither is a confirmed extra input surface.

When hardware bring-up is requested:

1. Read and preserve the existing `0x8047`–`0x8100` configuration before changing
   it. Record controller/firmware identity and the module ID at `0x814A`.
   Inspect the key, border, and channel-map settings together. The vendor's
   [`loadConfig`/`dumpRegister` methods][gt911-config] show a readback path.
2. Log ready touch/key reports while tapping across the lower bezel, including
   press/release, the marked Home area, and locations beside it. Keep raw sensor
   observations separate from rotated application coordinates.
3. If the readback and panel evidence support an experiment, change one setting
   at a time from the saved baseline and compare the response map. Follow the
   guide's section 4.4 checksum/update procedure; test ordinary screen accuracy
   as well. Do not assume a reset restores the original settings.

Until measured, expose Home as a logical key and leave extra bezel controls
unavailable. These findings do not add a driver or simulator model.

## Shared buses, power domains, and wake routing

| Resource | Consumers / wiring | Design constraint |
| --- | --- | --- |
| Main I2C, SDA39/SCL40 | Touch, RTC, expander, charger, fuel gauge, display supply | One serialized bus owner; failure/recovery can affect several services |
| Shared SPI, SCK14/MOSI13/MISO21 | SD CS12 and LoRa CS46 | Per-device configuration and exclusive transactions; bound SD work so radio servicing is not starved |
| Main `VDD3V3` | MCU, SD, expander, touch, logic | No independently controlled SD power switch is shown; sleep commands and full power removal differ |
| Auxiliary `VCC3V3` | LoRa and GNSS, controlled by PCA9535 `IO0_0` / `LORA_EN` | Both clients share one rail; neither driver may cut power while the other still needs it |
| Display high-voltage rails | TPS651851, commanded and observed through PCA9535 | Display driver owns sequencing, power-good/fault handling, and safe shutdown |
| Frontlight supply | Boost converter from `VSYS`, GPIO11 enable/PWM | May illuminate a retained image while panel scan power is off |
| RTC backup `VRTC` | PCF8563 and J12 / MS412FE source | Retention depends on a fitted, charged backup source; validate after system power loss |

These connections are from [schematic][schematic] sheets 1–4 and the
[vendor pin map][pinmap]. Charger `INT` and gauge `GPOUT` have no demonstrated
route to an MCU input in those sheets; begin with polling rather than promising
hardware battery notifications. The SD socket's `CD#` has no MCU connection in
sheet 2, so card insertion/removal also needs probing and I/O error detection.
Neither a shared rail enable nor an interrupt proves a device is ready.

| Event | Wake representation |
| --- | --- |
| BOOT GPIO0 | Candidate RTC-GPIO wake source; retain its separate reset/boot-strapping semantics |
| Function key via PCA IRQ38 | Can use ordinary GPIO wake in light sleep with the bus/expander available; GPIO38 is outside the ESP32-S3 RTC-GPIO wake set |
| Touch/Home via IRQ3 | Candidate RTC-GPIO wake if the touch controller remains powered and can assert its interrupt |
| RTC alarm via IRQ2 | Candidate RTC-GPIO wake after alarm programming and pending-flag handling |
| LoRa IRQ10 | Candidate RTC-GPIO wake while the radio and auxiliary rail remain in an appropriate receive mode |
| GNSS UART RX44 | UART light-sleep wake is a separate, backend-dependent capability; do not promise lossless receipt of the first message or deep-sleep wake |
| Reset, QON, USB power restoration | Board reset/power transitions, not resumable app input events |

ESP32-S3 external RTC-GPIO wake uses pins 0–21; normal GPIO light-sleep wake
has different constraints. Thus the function key is **not** an interchangeable
deep-sleep wake source for BOOT. The sleep planner must reject unsupported wake
requests, rather than silently selecting a mode that cannot wake as requested.
These are wiring-based candidates, not tested wake configurations.
[Espressif's sleep-mode reference][sleep] describes the silicon restrictions;
it is a hardware reference, not a decision to use an ESP-IDF application runtime.

## Pin-definition coverage

The [H752-01 pin definitions][pin-definitions-live] were compared with the pinned
file on 2026-10-12. The branch head matched the reviewed commit, and the files
were byte-identical. Every defined signal family has an owner:

| Pin-definition group | Inventory / service owner |
| --- | --- |
| GPS RX/TX and UART aliases | GNSS receiver / DEV-LOCATION |
| Main I2C and touch/RTC aliases | Shared board bus / backend resource coordinator; aliases are not extra buses |
| Main SPI and SD/LoRa aliases | Shared board bus / backend resource coordinator, arbitrated for DEV-FILES and DEV-RADIO |
| Touch reset and interrupt | Touch controller / DEV-INPUT |
| RTC interrupt | External RTC / DEV-TIME and DEV-POWER wake planning |
| LoRa CS, IRQ, reset, busy | SX1262 / DEV-RADIO |
| SD CS | Removable storage / DEV-FILES |
| Frontlight enable | DEV-LIGHT |
| BOOT and expander interrupt | Physical input / DEV-INPUT, with shared IRQ dispatch in backend resource coordinator |
| E-paper data, clock, latch, and start signals | Native display backend / DEV-DISPLAY |
| Expander port 1 output-enable/mode and TPS control/status | Display backend through backend resource coordinator; includes VCOM, power-good, and fault handling |
| Expander port 1 button | S3 Function / DEV-INPUT |
| Unnamed expander port 0 definitions | Schematic identifies bit 0 as the shared LoRa/GNSS rail; bits 1–7 are unconnected in the reviewed revision |

The macro list alone omits the non-GPIO reset/QON controls, capacitive Home key,
charger, gauge, USB wiring, and backup-power details. The schematic and examples
complete those parts of the inventory; unnamed definitions are not evidence of
additional peripherals or freely usable board connections.

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
| Function button | PCA9535 `IO1_2` / index 10; shared expander IRQ38 |
| Reset button | ESP32 `EN`, not a GPIO |
| QON button | BQ25896 `QON`, not a GPIO |

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
5. Identify all physical controls, the touch variant/Home key, GNSS variant, and
   backup battery. Verify wake sources separately for each intended sleep mode.
6. Exercise SD removal, simultaneous SD/radio activity, auxiliary-rail sharing,
   USB-versus-battery shutdown, and recovery from reset during I/O.

The firmware currently leaves board peripherals uninitialized; that does not
guarantee they are powered off. Timer waits yield to the scheduler, but automatic
light sleep and deep sleep are not configured. Measure power on the real board
before drawing conclusions about battery life. MeshCore integration, multicore
work allocation, persistent storage, and an OTA partition/update design remain
separate steps.

## Backend conformance scenarios

These are planned additions to the [portable acceptance scenarios](../device-interfaces.md#simulator-acceptance-scenarios),
not implemented simulator controls or verified hardware behavior. A simulator
preset claiming this board must model these constraints. Synthetic profiles
with other capabilities remain useful for testing the portable application.

| Board stimulus | Expected mapping to shared contracts |
| --- | --- |
| S1 reset during SD write or display refresh | New application epoch; interrupted work can be lost; panel content is not automatically blank and driver history can be unknown |
| S2 BOOT bounce, held at startup, or held across reset | Normal runtime edges use shared gestures; startup synchronization avoids a phantom click; reset with BOOT held can select ROM download mode with no running app |
| S3 function and TPS fault assert shared PCA IRQ38 | Dispatch actual inputs; no spurious button event, lost fault, or overwritten output state |
| GT911 Home and optional GT6972P pen | Advertise only verified event kinds/tool capabilities; no invented bezel coordinates |
| S4 QON short/long hold, battery-only versus USB | Apply charger-specific power/reset behavior; no generic app power-key callback |
| GNSS searching/lost fix or either receiver variant | Same validity/age contract; independent position and time validity |
| GNSS stops while LoRa receives | Shared auxiliary rail stays enabled for the radio |
| SD insertion/removal/replacement during I/O | Probe/error detection latency; stale mount handles/completions rejected |
| SD traffic competes with LoRa | Bounded SPI transactions and observable radio backpressure |
| RTC backup loss or UTC correction | Invalid civil time reported without moving monotonic deadlines |
| USB attached while preparing battery shutdown | Revalidate the plan; never report confirmed power-off merely from accepting a command |
| Function key requested as the only deep-sleep wake source | Reject the unsupported plan; an alternative requires an explicit policy choice |
| Frontlight changes while EPD scan power is off | Illumination can change independently of panel refresh |
| Shared I2C fails and recovers | Cancel uncertain input; mark dependent data stale/unavailable and restore resource knowledge |
| Automatic rotation requested on the baseline board | Report unavailable source; manual orientation remains a supported design path |

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
