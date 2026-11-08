# Discovery and communications

[Device services](../device-interfaces.md)

## Current discovery interface

`Discovery` implements the scan operation for Wi-Fi and Bluetooth LE.
The [radio debugger](../application.md#radio-debugger) selects scan requests and display behavior.
It returns names, addresses, address types, and optional RSSI in dBm.
A completion includes its request and observation time.
The adapter identifies simulated or physical observations when it accepts a request.
Current adapters provide simulated observations, physical observations, and unsupported operation reports.

`ScanResults` stores up to 24 entries without heap allocation.
Wi-Fi results identify access points by BSSID.
Bluetooth LE results identify devices by address and address type.
A new report replaces the previous report for the same identity.
Different identities keep different entries, even when names are the same.

The buffer sorts measured RSSI from highest to lowest, then puts unknown RSSI last.
Thus, -38 dBm precedes -73 dBm.
Equal values use address order, then address type.
After overflow, the buffer keeps the strongest entries and sets `truncated`.

Names have a maximum length of 32 bytes of printable ASCII.
Other bytes become question marks. Raw name bytes stay outside the current contract.

The consumer selects the deadline in each `ScanRequest`.
The platform contract has no fixed scan timeout.
Accepted replacement requests keep their deadline during resource release.

The adapter writes completed results into storage that the consumer supplies.
It leaves that storage unchanged when no completion is available.
Errors clear that storage.

Cancellation invalidates a request immediately.
The adapter releases resources before completion delivery or the next physical scan.
Resource release can finish asynchronously after cancellation.
It must discard completions from an earlier application session.
The consumer accepts only completions for its active request.

Busy, unsupported, failure, and timeout results are separate errors.
An empty successful scan is not an error.
Connection, pairing, and application data transfer stay outside this operation.

## Reply transport

The platform [`ReplyTransport`](../../crates/platform/src/transport.rs) contract separates command handlers from their reply transport.
It exposes `is_session_active(session)` and an asynchronous reply operation.
The named `SessionId` type identifies the connection that supplied the request.
The backend assigns identities, which callers treat as opaque and scoped to that transport.
A connection check does not reserve the session. The send operation must also reject stale sessions.
Replies must not continue into a replacement session after disconnect or reconnect.

Successful send completion means that the transport sent the complete reply, rather than only accepting it into a queue.
It does not establish that the remote application processed the reply.
The backend bounds the wait and invalidates pending writes when the send future is dropped.
Cancellation cannot recall bytes already transmitted.
The associated `Reply` type lets the backend select storage without a platform allocation requirement.
Replies expose bytes so the protocol can select text or binary encoding.

The firmware USB backend implements the contract with owned strings and its existing CDC session tracking.
The runtime supplies command dispatch to the USB worker.
ROM download and display inspection handlers receive the reply transport instead of calling USB directly.
The runtime retains application pause and display hold ownership until the response completes.
The [board USB profile](../boards/lilygo-t5-s3-pro-usb.md) defines hardware behavior, protocol details, and timeout values.

Native tests cover connection changes, stale replies, canceled sends, acknowledgement ownership, and command parsing.
Firmware compilation checks the connection between the platform interface, handlers, and USB backend.
Hardware transport behavior requires device validation.

## Request framing

The platform [line framer](../../crates/platform/src/transport/framing.rs) collects bounded byte frames terminated by LF.
Callers select storage capacity and can set a smaller protocol limit with `with_limit`.
Configuration rejects limits above storage capacity. Default construction uses the full capacity.
The delimiter does not consume payload capacity and is absent from the returned frame.
Other bytes, including CR, remain unchanged. Protocol code decides whether an empty frame is valid.
Overflow discards input through the next LF, then returns `FrameError::TooLong` and permits a new frame.
The caller must clear partial input and overflow state when its source session changes.

The firmware runtime supplies the command size limit to USB startup.
USB validates that limit against its fixed buffer capacity before it starts workers.
Firmware retains JSON parsing, error responses, and command routing.
USB passes framing errors to the runtime callback with the original session identity.
The same session checks apply to error replies and successful command replies.
Platform tests cover capacity and protocol limits, overflow recovery, session clearing, empty frames, and byte preservation.
Native integration tests combine the public framer with firmware routing to check fragmented commands and disconnects.

## Planned communications services

Packet radio, general connection management, file transfers, and update transport stay planned work.

`PacketRadio` reports supported PHY settings and payload limits.
Received packets include supported signal metadata.
Transmit results identify the request.
Transmit completion does not confirm peer delivery.
Overflow, deadlines, reset, and availability changes must have observable failure
results. Shared software selects mesh protocols independently of the radio driver.

Host and network transports report their capabilities and session state.
Features that depend on a connection stay unavailable without that connection.
File export must control media ownership.
Host and local filesystem writers must not write the same media at the same time.
OTA and update policy stay separate design work.
