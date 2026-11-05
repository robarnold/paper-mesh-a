# Simulator API

The browser and scripted clients use the same device session. The [device routes and commands](../../simulator/src/main.rs)
define the HTTP interface. The [simulation snapshot](../../simulator/src/lib.rs) defines the device observations.

To reproduce a timing boundary, pause wall-clock pacing before advancing virtual time.
For example, send these JSON bodies in order to `POST /api/event` with `Content-Type: application/json`:

1. `{"type":"pause"}` stops automatic advances.
2. `{"type":"advance","ms":1000}` processes the next second of virtual time, including intermediate deadlines.

Read `GET /api/state` for application state and `GET /frame.png` for the last completed frame.
A pending refresh can make the two differ. Send `{"type":"resume"}` to restore interactive pacing.

## UI guide

Each guide example accepts events separately from the device session.
Send an input sequence to the same example whose state and frame you inspect.
The [guide routes](../../simulator/src/ui_guide_http.rs) identify each example and its endpoints.
The calculator uses `/ui-guide/calculator/state`, `/ui-guide/calculator/event`, and `/ui-guide/calculator.png`.
Device snapshots also report calculator visibility, value, status text, and resolved key targets.

## Pointer conversion

Send device touch coordinates in the portrait viewport. Use the same contact ID from press through release.
The simulator applies the shared rotation before hit testing. Native tests can inject controller coordinates to check that mapping.
See the [contact scenarios](../../simulator/tests/contacts.rs).

## Host touchpad observations

Use the simulated host's report and detachment counts to distinguish active application contacts from reports the host accepted.
The mini touchpad shows contact positions even though the simulator sends no events to the desktop.
See the [touchpad scenarios](scenarios.md#usb-touchpad).
