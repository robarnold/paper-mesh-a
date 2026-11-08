# ESP USB adapter

The [ESP adapter crate](../../crates/esp-adapters/README.md) implements composite CDC diagnostics and HID touchpad transport.
It currently supports the ESP32-S3 USB OTG peripheral through esp-hal and Embassy.
Support for other ESP chips is not established.

## Ownership

The adapter owns USB buffers, endpoint settings, workers, sessions, and worker pause channels.
It implements the platform touchpad, reply transport, suspend, and power-off contracts.
The [worker coordination document](../platform/worker-coordination.md#usb-integration) defines pause order and restoration.

The runtime supplies the peripheral, pins, USB identity, HID descriptor, request limit, command callback, and diagnostic providers.
The [board profile](../boards/lilygo-t5-s3-pro-usb.md) defines the current wiring, identity, geometry, and host behavior.
The descriptor must match the logical coordinates submitted through the touchpad contract.
The adapter has no dependency on firmware JSON parsing, application screens, or system power coordination.

Startup uses static storage for one device and installs the diagnostic logger.
It must occur once, after executor and allocator initialization.
The adapter validates the request limit before it starts workers.
The [framing contract](../services/connectivity.md#request-framing) and [reply transport contract](../services/connectivity.md#reply-transport) define portable behavior.

## Build and verification

The adapter belongs to the native workspace with no default features.
Firmware enables `esp32s3`, which selects optional hardware dependencies and the device implementation.
Native tests compile the actual session and lifecycle modules inside their owning crate.
They do not import private source files from another crate.
Lifecycle tests substitute connection notifications and use a single-thread mutex without USB peripherals.

The [full checks](../../justfile) run native unit tests and explicitly select the hardware library for firmware Clippy.
Firmware image builds link the hardware implementation with runtime configuration.
These checks do not establish physical suspend, reconnection, or host compatibility.

The crate boundary gives Cargo a separate compilation unit. Release builds retain whole-program optimization.
No build-time improvement has been measured.
