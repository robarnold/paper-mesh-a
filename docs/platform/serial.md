# Serial receive sessions

The platform [serial contract](../../crates/platform/src/serial.rs) separates byte reception from peripheral ownership.
A serial owner lends one exclusive session implementing embedded I/O `Read`.
Firmware selects pins, framing, and hardware settings. Opening a session selects its baud rate.

Each successful open discards buffered input and receive errors from earlier sessions.
Failure returns no session and permits retry. The exclusive borrow prevents another session or lifecycle operation while reception owns the peripheral.
Closing a session releases reception resources. It does not establish system sleep readiness.

## Cancellation and deadlines

A pending read need not preserve bytes when canceled.
A consumer that cancels a read must end the session and discard partial protocol state before restarting.
Ordinary observation deadlines retain the same pending read. Only session-ending events abandon it.
The backend must safely stop access to borrowed buffers when reads are dropped.

The [GNSS acquisition loop](../../crates/gnss/src/acquisition.rs) retains a pending read across satellite expiry.
Silence, end-of-stream, and receive errors end acquisition. The runtime closes that session before recovery.
No baud changes occur inside normal acquisition.

## Hardware integration

The [ESP serial adapter](../adapters/serial.md) owns UART resources and implements fresh receive sessions.
Firmware selects the peripheral, pins, settings, and executor. It coordinates hardware sleep preparation after reception closes.
Hardware preparation remains separate from the portable receive contract because closing a session does not prove system sleep readiness.

## Verification and dependencies

Native [session tests](../../crates/gnss/tests/sessions.rs) cover bounded probing, canceled input, failed opens, receive errors, and reads retained across observation deadlines.
Firmware checks compile the concrete session and sleep guard with stack limits enabled.
Physical power-cycle retention, wake behavior, and UART timing remain hardware verification work.

The [platform manifest](../../crates/platform/Cargo.toml) pins `embedded-io-async`.
The [workspace lockfile](../../Cargo.lock) and [firmware lockfile](../../firmware/Cargo.lock) also pin its `embedded-io` dependency.
Both packages come from [rust-embedded/embedded-hal](https://github.com/rust-embedded/embedded-hal) and permit MIT or Apache-2.0 licensing.
This project uses their MIT terms. Distribute the [async notice](../../crates/platform/licenses/LICENSE-embedded-io-async-MIT) and [I/O notice](../../crates/platform/licenses/LICENSE-embedded-io-MIT) with binaries that include them.
