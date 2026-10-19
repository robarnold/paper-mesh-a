# Host touchpad service

[Device services](../device-interfaces.md) · [Application mode](../application.md#usb-touchpad)

`TouchpadHost` accepts complete contact states with monotonic timestamps.
It reports disconnected, ready, suspended, or fault state.
Its generation changes after a transport reset, including a reset between consumer polls.
The application selects when the service is active.
The firmware USB adapter and native simulator implement this contract.

`TouchpadSession` consumes normalized `TouchFrame` values before ordinary application controls.
It maintains up to five contacts with IDs from 0 through 15.
Active sessions capture raw movement and releases. The host recognizes gestures.
Mode changes and transport changes suppress held contacts until all contacts lift.
The activation tap therefore cannot become a host click.

Submission is nonblocking and bounded.
Rejection or invalid input cancels host state. A transport must not silently lose a release.
The USB backend detaches to clear host gesture state, including an active drag lock.
The simulator records detachments and accepted frames.

The [USB backend](../boards/lilygo-t5-s3-pro-usb.md) defines HID reports, buffering, recovery, and hardware limits.
The [simulator scenarios](../simulator/scenarios.md#usb-touchpad) cover mode changes, contact IDs, and connection changes.
