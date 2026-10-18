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

## Planned communications services

Packet radio, connections, file transfers, and update transport stay planned work.

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
