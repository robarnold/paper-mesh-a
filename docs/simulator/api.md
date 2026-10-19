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
| `touch_down`, `touch_move`, `touch_up` | `x`, `y`, optional `id` from 0 through 15. The default ID is 0. |
| `touchpad_host` | `state`: `ready`, `disconnected`, `suspended`, or `fault` |
| `hardware_button` | `id`: 0 for BOOT, 1 for Function, or 2 for Home. `pressed`: boolean. |
| `touch_cancel`, `pause`, `resume`, `reset` | None |
| `advance` | `ms`, limited to 1–60000 ms per request with a paused clock |
| `scan_scenario` | `scenario`: `nearby`, `empty`, `failed`, `unavailable`, or `timeout` |
| `location_scenario` | `scenario`: a value from the [GNSS scenarios](scenarios.md#gnss-screen) |
| `power_scenario` | `scenario`: a value from the [power scenarios](scenarios.md#power-screen) |

The browser provides equivalent buttons for device touch targets.
It also shows hardware controls from `hardware_buttons` in the state response.
Hardware samples represent debounced input. Reset uses the existing `reset` event.
QON has no readable input and stays disabled in the simulator.
Rust supplies their labels and coordinates through `/api/state`.
These buttons also permit keyboard input.

`GET /api/state` includes `utc_datetime` as a UTC timestamp string or `null` when calendar time is unknown.
The `now_ms` value remains monotonic during calendar corrections.

Power fields include the selected scenario, open screen, selected tab, and each source's data status.
Battery percentage, presence, voltage, current, source, and charge phase use `null` when unavailable.
The device screen shows the complete power details.

## Pointer conversion

The browser converts pointer positions into portrait display coordinates.
It retains up to five concurrent pointer IDs. `contacts` in the state response lists active IDs and logical coordinates.
`Simulation::touch_portrait` converts those positions into native contacts.
It passes them to `touch_native`.
This method applies shared input rotation before application hit testing.
Native scenarios can call `touch_native` directly with coordinates aligned to
the controller.

## Host touchpad observations

`touchpad_open` reports the application mode. `touchpad_host` reports the simulated connection state.
`touchpad_reports` counts accepted contact states. `touchpad_detachments` counts cancellations.
The mini touchpad renders `contacts` with their IDs and portrait positions.
It appears only while the application mode is open.
It does not send input to the computer's desktop.
