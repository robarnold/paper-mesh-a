# Native Rust simulator

The simulator runs the application with native implementations of device
services. It does not emulate ESP32-S3 instructions or registers.
The first screen contains a touch counter and a 30-second heartbeat.
Its `Radio debugger` button opens Wi-Fi and Bluetooth LE scans with test data.
It uses the same state machine and rendering code available to firmware.

The current demonstration uses a 540 × 960 portrait coordinate space.
Staging, pending, and visible framebuffers use native 960 × 540 row-major storage.
Drawing and touch use the shared portrait rotation adapter.
The browser displays a portrait view of the completed native frame.
This rotation is fixed.

The [application architecture](architecture.md#orientation-state-and-coordinate-spaces)
gives information about planned mounting configuration, automatic orientation, manual locks,
and rotation state associated with frames.

## Code boundaries

| Location | Responsibility |
| --- | --- |
| `crates/app` | `no_std` application state, touch handling, deadlines, rendering |
| `crates/app/src/platform.rs` | Clock and display contracts, normalized touch events |
| `crates/app/src/platform/discovery.rs` | Scan contract and bounded result order |
| `crates/app/src/debugger.rs` | Shared debugger controls and rendering |
| `simulator/src/discovery.rs` | Deterministic scan results and failures |
| `crates/app/src/platform/rotation.rs` | Shared pixel and touch transforms |
| `simulator/src/lib.rs` | Virtual clock, grayscale display, refresh completion, deterministic scenarios |
| `simulator/src/main.rs` | Native HTTP server, input checks, optional wall-clock pacing |
| `simulator/src/index.html` | Browser transport and simulation controls. No application logic or rendering. |
| `firmware/src/main.rs` | ESP startup, Embassy scheduling, clock adapter |

The root workspace builds on native Linux.
`firmware/` is a separate Cargo project with its own lockfile, toolchain, and
target configuration. It uses a path dependency for the shared application.
This separation prevents accidental combination of native and ESP executor,
platform, and dependency features.

## Shared interfaces

The [application architecture](architecture.md) defines clock, touch, display,
rotation, scheduling, and ownership contracts shared with hardware.
The simulator implements these contracts without ESP registers or an executor
dependency.

Firmware uses `Clock`, the shared heartbeat state machine, and physical scan adapters.
Native tests also exercise the firmware's request state and Bluetooth advertisement conversion.
These tests do not verify radio hardware.
It has no placeholder display or touch drivers that report incorrect hardware
success. Cross-compilation includes the shared renderer.
Physical rendering must have a panel driver, buffers, and power sequences.

## Scenario profiles and board conformance

The simulator implements portable services.
Its current geometry matches the first hardware target.
Future scenarios can represent different devices.
Configurable profiles stay planned work.

- Contract scenarios change geometry, controls, optional services, display
  capabilities, latency, and faults.
  They verify that application behavior uses declared capabilities without
  dependence on a particular SKU.
- Board conformance presets apply a physical backend's resource and lifecycle
  constraints to the same contracts.
  The [LILYGO conformance cases](boards/lilygo-t5-s3-pro.md#backend-conformance-scenarios)
  cover its buttons, shared resources, wake limits, and power behavior.

A synthetic profile with a sensor can verify automatic orientation.
Its label must not claim a match to a board without that sensor.
Neither profile type proves electrical timing or physical driver correctness.

## Deterministic execution

`Simulation::advance` visits refresh and heartbeat deadlines in time order.
One 90-second advance gives the same observable result as many small advances.
Tests do not sleep or depend on wall-clock speed.
Interactive mode uses wall time to pace the same virtual clock.
Pause disables this pacing.
Reset keeps the current paused or active setting.

The display starts white.
Refresh has an artificial delay of 250 ms.
The last image stays visible until completion replaces it.
This atomic replacement simplifies simulation.
Physical e-paper changes throughout its drive sequence.

[Display constraints](display-constraints.md) give information about planned damage costs,
waveform phases, and region scheduling.
Pixels have 16 grayscale levels.
PNG export keeps these values in an 8-bit grayscale image.
Idle polling does not refresh the screen.
Touch changes and heartbeat updates cause refreshes.

Tests cover input cancellation, bounds, heartbeat intervals, and frame visibility.
They also cover changes during refresh, failure and retry, equivalent time
advances, and PNG output.
Rotation tests check asymmetric native pixel layouts.
They compare the completed portrait view with independent direct rendering.
`just ci` produces `target/simulator/hello.png` and `target/simulator/native.png`
for visual checks of both orientations.

## Running and extending

Run `devcontainer exec --workspace-folder . just sim`.
Open `http://localhost:8080`.
The container publishes port 8080 only on host loopback.
The HTTP server uses the native Rust toolchain in the devcontainer.
No browser framework, Node, SDL, or host GUI dependencies are necessary.

The development API has these endpoints:

| Endpoint | Result or input |
| --- | --- |
| `GET /api/state` | Current simulation state |
| `GET /frame.png` | Portrait frame |
| `GET /frame-native.png` | Native frame |
| `POST /api/event` | Event body with `Content-Type: application/json` |

Event bodies use a `type` field:

| Type | More fields |
| --- | --- |
| `touch_down`, `touch_up` | `x`, `y` |
| `touch_cancel`, `pause`, `resume`, `reset` | None |
| `advance` | `ms`, limited to 1–60000 ms per request with a paused clock |
| `scan_scenario` | `scenario`: `nearby`, `empty`, `failed`, `unavailable`, or `timeout` |

The browser provides equivalent buttons for device touch targets.
Rust supplies their labels and coordinates through `/api/state`.
These buttons also permit keyboard input.

### Radio debugger

1. Select `Radio debugger` on the device screen or in the browser controls.
2. Select `Wi-Fi` or `Bluetooth LE`.
3. If virtual time is paused, advance it by two seconds.
4. Use `Next` and `Previous` to see more results.
5. To repeat the scan, select `Scan again`.
6. To return to the counter, select `Back`.

The debugger shows RSSI in dBm, with the strongest measured signal first.
Unknown RSSI appears last.
The scan time identifies the completed results.
Wi-Fi names can repeat across different access points.
Bluetooth LE results show devices that advertise.
The debugger does not support Bluetooth Classic discovery.

The `Radio scenario` selector changes the next scan.
The nearby scenario has seven entries for each radio, including an empty name and unknown RSSI.
Other scenarios produce empty results, a failure, an unavailable adapter, or a timeout.
Reset restores the nearby scenario and the counter screen.
The simulator does not access host radios or physical device radios.

The browser converts pointer positions into portrait display coordinates.
`Simulation::touch_portrait` converts those positions into native contacts.
It passes them to `touch_native`.
This method applies shared input rotation before application hit testing.
Native scenarios can call `touch_native` directly with coordinates aligned to
the controller.

For the first consumer of a radio, storage, or power service, add the shared contract.
Add a native implementation with a behavioral scenario.
Keep physical chip protocols in drivers below these services.

The simulator does not model radio propagation, battery current, sleep or ULP
execution, core scheduling, panel waveforms, ghosting, temperature, or DMA timing.
Its refresh delay is a test condition.
It is not a measured hardware characteristic.

The [device service design](device-interfaces.md#simulator-acceptance-scenarios)
lists portable contract scenarios.
Board profiles add physical control mappings.
The current Reset action resets the test fixture.
It does not model hardware reset, power controls, or persistence across power
transitions. GNSS, SD, battery, charger, frontlight, RTC, packet radio, and physical
buttons have no simulator implementations yet.
