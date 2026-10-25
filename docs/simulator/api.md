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

Each entry in `controls` includes `enabled`, which the browser uses to disable unavailable controls.
Header controls use the same bounds and enabled state as the application.

`diagnostics_open` is true in the Diagnostics menu and its component pages.
`diagnostic_component` identifies `Display`, `Touch`, `Radio`, `GNSS`, or `Power`. It is `null` outside a component page.
Display checks also supply `diagnostic_pattern` and `diagnostic_ready`.
The pattern is `black`, `white`, `gray`, or `null` outside a pattern.
Readiness confirms completion of the simulated refresh.
The `full_refreshes` count records accepted requests for a complete refresh.

## UI guide

Each section of the [visual guide](execution.md#ui-guide) has a separate interactive example.
Their events do not change device application state or each other.

| Endpoint | Result or input |
| --- | --- |
| `GET /ui-guide` | Browser guide |
| `GET /ui-guide/state` | Shell and Page state, complete back stack, return destination, lifecycle counts, and equivalent controls |
| `GET /ui-guide/frame.png` | Current Shell and Page example |
| `GET /ui-guide/controls/state` | Controls state, action count, selection, and equivalent controls |
| `GET /ui-guide/controls.png` | Current Controls example |
| `GET /ui-guide/text/state` | Selected text role, underline and bubble states, spacing, color role, and equivalent controls |
| `GET /ui-guide/text.png` | Current Text and spacing example |
| `GET /ui-guide/catalog.png` | Static visual catalog |
| `GET /ui-guide/snippets` | Named Rust excerpts from the compiled example source |
| `GET /ui-guide/source.rs`, `/ui-guide/controls.rs`, `/ui-guide/text.rs`, `/ui-guide/catalog.rs` | Complete example sources |
| `GET /ui-guide/reference/shared`, `/ui-guide/reference/application`, `/ui-guide/reference/toolkit` | Repository documentation as Markdown |
| `POST /ui-guide/event` | Shell and Page command, with the updated state as its response |
| `POST /ui-guide/controls/event` | Controls command, with the updated state as its response |
| `POST /ui-guide/text/event` | Text and spacing command, with the updated state as its response |

Guide commands use a `type` field.
`touch_down`, `touch_move`, and `touch_up` require `id`, `x`, and `y`.
Contact IDs range from 0 through 15. Coordinates must be inside the 540 × 960 viewport.
`cancel`, `home`, `finish`, and `reset` have no additional fields.
The host routes `home` through the platform's reserved Home binding and suppresses held contacts.
`finish` completes the example's pending operation. `reset` resets only the example at that endpoint.
The Text and spacing example has no pending operation and ignores `finish`.
Its `underlined` and `bubbled` fields apply independently to the selected text role. Reset clears both selections.
In the Shell and Page state, `return_to` identifies the latest caller.
It is null at Home after launch or a Home request.
`back_stack` lists all return destinations, with the oldest caller first and the next Back destination last.
It excludes the current page and is empty at Home or after reset.

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
