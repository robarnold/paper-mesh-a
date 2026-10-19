# Input and lifecycle controls

[Device services](../device-interfaces.md)

`input::buttons` supplies hardware control descriptors, timestamped events, and a debounce component.
Descriptors distinguish edge input, activation input, reset, and power recovery.
Reset and power recovery descriptors do not imply readable input events.
Firmware and simulator profiles assign identities. Applications select bindings separately.
Initial samples and recovery synchronize the level without an invented press.
Backends select debounce intervals from the electrical input requirements.

Detailed lifecycle reports stay planned work.
The current interface supplies `Touch`, `Release`, and `Cancel` events.
Each contact has a stable ID and a position. IDs range from 0 through 15.
`Touch` supplies both initial contact and subsequent movement.
`Release` retains the last valid position for that ID.
`Cancel` invalidates all contacts without a successful gesture.
The platform's `TapGesture` recognizes taps without filtering the raw stream for other consumers.
`TouchFrame` groups one observation with its timestamp and up to ten contact changes.
It permits five releases and five replacement contacts in one frame.
`ContactState` applies a frame atomically and retains up to five active contacts.
An invalid frame cancels the complete state.

A backend supplies stable control identities, labels, and supported event kinds.
An identity is separate from an action binding such as back, select, or sleep.
Profiles can supply default bindings.

Platform components supply gesture recognition. Applications select action bindings.
Display rotation does not change a control's identity.
The design uses no universal list of enclosure buttons.

- Controls with edge reports supply debounced `Pressed` and `Released`
  observations with timestamps. Shared policy derives each gesture once.
  Startup synchronization detects held controls without an incorrect click on release.
- Controls with activation reports supply `Activated`.
  An activation callback cannot establish accurate hold or repeat behavior.
  A separate capacitive key produces a control event.
  It does not produce an invented display coordinate.
- Input loss, reset, driver restart, or overflow cancels armed gestures.
  An orientation transition cancels gestures that depend on coordinates.
  Button state stays independent of pointer coordinates.
- Multiple contacts, pressure, tilt, hover, pen buttons, and eraser state are
  optional capabilities. Their implementations must keep contact and tool
  identity. Pressure, tilt, hover, pen buttons, and eraser state stay planned work.

Some physical controls operate outside the application.
They can force a reset, remove power, or select a boot mode.
The backend models these lifecycle effects without invented key callbacks.
The application receives only observable boot or reset information.
It receives `Unknown` when the backend cannot identify the cause.

A simulator trace can record an external stimulus that the application cannot
observe. Backend conformance tests give information about particular boot sequences.
