# Native Rust simulator

The simulator runs the application against native implementations of device
services. It is not an ESP32-S3 instruction or register emulator. The first
screen is deliberately small: a touch counter and a 30-second heartbeat, sharing
the same state machine and rendering code that the firmware can use.

The application uses a 540 × 960 portrait coordinate space. The simulator's
staging, pending, and visible framebuffers use native 960 × 540 row-major
storage. Drawing and touch pass through the shared portrait rotation adapter.
The browser displays a portrait view derived from the completed native frame.
This is currently a fixed effective rotation. The planned shared mounting,
automatic/locked orientation policy, and frame-associated rotation state are
described in [the application architecture](architecture.md#orientation-state-and-coordinate-spaces).

## Code boundaries

| Location | Responsibility |
| --- | --- |
| `crates/app` | `no_std` application state, touch handling, deadlines, and rendering |
| `crates/app/src/platform.rs` | Clock and display contracts, normalized touch events |
| `crates/app/src/platform/rotation.rs` | Shared pixel and touch coordinate transformations |
| `simulator/src/lib.rs` | Virtual clock, grayscale display, refresh completion, deterministic scenarios |
| `simulator/src/main.rs` | Native HTTP server, input validation, and optional wall-clock pacing |
| `simulator/src/index.html` | Browser transport and simulation controls; no application logic or rendering |
| `firmware/src/main.rs` | ESP startup, Embassy scheduling, and the clock adapter |

The root workspace builds on native Linux. `firmware/` is a separate Cargo
project with its own lockfile and toolchain/target configuration. It depends on
the shared application through a path dependency. This keeps native and ESP
executor, platform, and dependency features from being combined accidentally.

## Shared interfaces

The [application architecture](architecture.md) defines the clock, touch, display,
rotation, scheduling, and ownership contracts shared with hardware. The simulator
implements those contracts without depending on ESP registers or an executor.

Firmware currently exercises only `Clock` and the shared heartbeat state
machine. There are no placeholder display/touch drivers claiming hardware
success. The shared renderer is cross-compiled, but physical rendering awaits
the actual panel driver, buffering, and power sequencing.

## Deterministic execution

`Simulation::advance` visits refresh and heartbeat deadlines in chronological
order. Advancing 90 seconds at once has the same observable result as many
small steps. Tests never sleep or depend on wall-clock speed. Interactive mode
uses native wall time to pace that same virtual clock; Pause disables pacing.
Reset preserves the current paused/running setting.

The display starts white. Refresh takes an artificial 250 ms, during which the
previous image remains visible. Only completion replaces the visible frame.
This atomic replacement is a simulation simplification: a physical e-paper
panel changes during its drive sequence. Damage-dependent cost, waveform phases,
and region scheduling are described as future work in
[the display constraints](display-constraints.md).
Pixels have 16 grayscale levels, exported losslessly as an 8-bit grayscale PNG.
Idle polling does not refresh the screen; touch changes and heartbeat updates do.

Tests cover input cancellation and bounds, heartbeat cadence, frame visibility,
changes arriving during a refresh, refresh failure/retry, fast-forward
equivalence, and PNG output. Rotation tests check asymmetric native pixel layouts
and compare the completed portrait view with an independent direct rendering.
`just ci` generates `target/simulator/hello.png` and `target/simulator/native.png`
for visual inspection of both orientations.

## Running and extending

Use `devcontainer exec --workspace-folder . just sim`, then open
`http://localhost:8080`. The container publishes port 8080 on host loopback only.
There are no browser framework, Node, SDL, or host GUI dependencies. The HTTP
server runs on the existing native Rust toolchain in the devcontainer.

The small development API consists of `GET /api/state`, `GET /frame.png`
(portrait), `GET /frame-native.png` (native storage), and `POST /api/event` with
`Content-Type: application/json`. Event bodies use a
`type` field: `touch_down`/`touch_up` with `x` and `y`, `touch_cancel`, `pause`,
`resume`, `advance` with `ms`, or `reset`. Advances require a paused clock and
are limited to 1–60000 ms per request. The UI converts pointer coordinates to
portrait display coordinates. `Simulation::touch_portrait` generates native
contacts from them and feeds `touch_native`, which applies the shared input
rotation before hit testing in the application. Native scenarios can call
`touch_native` directly to exercise controller-aligned coordinates.

Add application-facing radio, storage, or power services when their first real
consumer exists, along with a native implementation and a behavioral scenario.
Keep physical chip protocols in drivers beneath those services. The current
simulator does not model radio propagation, battery current, sleep/ULP execution,
core scheduling, panel waveforms, ghosting, temperature, or DMA timing. Its
refresh delay is a test condition, not a measured hardware characteristic.
