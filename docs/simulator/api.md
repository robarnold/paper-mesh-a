# Simulator API

[Simulator overview](../simulator.md)

Start the simulator with the [startup procedure](../simulator.md#use-the-simulator).
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
| `location_scenario` | `scenario`: a value from the [GNSS scenarios](scenarios.md#gnss-screen) |

The browser provides equivalent buttons for device touch targets.
Rust supplies their labels and coordinates through `/api/state`.
These buttons also permit keyboard input.

## Pointer conversion

The browser converts pointer positions into portrait display coordinates.
`Simulation::touch_portrait` converts those positions into native contacts.
It passes them to `touch_native`.
This method applies shared input rotation before application hit testing.
Native scenarios can call `touch_native` directly with coordinates aligned to
the controller.
