# Native Rust simulator

The simulator supplies native platform services and deterministic execution.
It runs the same application code as firmware.
The browser transports input and displays completed frames.
It does not contain application policy or rendering code.

The [architecture](architecture.md) defines code boundaries.
The [device services](device-interfaces.md) specify portable contracts.
The [application](application.md) describes screens and policy choices.
Hardware drivers and simulator adapters implement the same contracts.

## Use the simulator

Run `devcontainer exec --workspace-folder . just sim`.
Open [localhost:8080](http://localhost:8080).
Use Pause and the time controls to inspect application behavior.
To stop the server, press Ctrl+C.

| Topic | Contents |
| --- | --- |
| [Execution](simulator/execution.md) | Current geometry, virtual time, refresh completion, and frame export |
| [API](simulator/api.md) | HTTP endpoints, input events, and pointer conversion |
| [Scenarios](simulator/scenarios.md) | Radio and GNSS controls, test data, and expected results |
| [Development environment](development.md) | Devcontainer setup and host access |

## Planned profiles and conformance

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

## Limits

The simulator does not emulate processor instructions or registers.
The simulator does not model radio propagation, battery current, sleep or ULP
execution, core scheduling, panel waveforms, ghosting, temperature, or DMA timing.
Its refresh delay is a test condition.
It is not a measured hardware characteristic.

The [device service design](platform/service-validation.md)
lists portable contract scenarios.
Board profiles add physical control mappings.
The current Reset action resets the test fixture.
It does not model hardware reset, power controls, or persistence across power
transitions. Battery and charger scenarios supply fixed observations without electrical simulation.
SD, frontlight, packet radio, and physical buttons have no simulator implementations yet.
The [calendar scenario](simulator/scenarios.md#calendar-clock) supplies UTC without oscillator simulation.
