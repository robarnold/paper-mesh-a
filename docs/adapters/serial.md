# ESP serial adapter

The [serial adapter](../../crates/esp-adapters/src/serial.rs) implements the platform [receive-session contract](../platform/serial.md).
Firmware supplies the UART peripheral, RX pin, TX pin, and hardware settings. The adapter retains these resources without selecting an executor.
Its constructor requires an input-capable RX pin and an output-capable TX pin.
It stores type-erased HAL resources so worker types do not depend on the selected peripheral or pin numbers.

Each session constructs an async UART on the caller's executor with the requested baud rate.
HAL initialization resets the FIFOs. Dropping the session releases RX and its wake lock.
The exclusive session borrow prevents another session or sleep preparation from using the same UART.

## Sleep preparation

The runtime closes reception before calling `prepare_sleep`.
This creates a pinless idle transmitter that keeps the selected UART clocked during ESP sleep preparation.
The returned guard borrows the adapter, preventing reception until the guard is dropped.
Preparation errors remain visible to suspend coordination. The adapter does not select sleep policy or control the shared auxiliary rail.

## Pin selection

The ESP32-S3 GPIO matrix permits UART routing beyond the default UART pins.
Firmware must select pins compatible with board wiring, boot configuration, memory connections, and other active peripherals.
HAL pin capabilities do not establish board availability.
See Espressif's [UART routing](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-reference/peripherals/uart.html) and [GPIO restrictions](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-reference/peripherals/gpio.html).

## Verification limits

Firmware checks compile the adapter through the GNSS worker with stack limits enabled.
Native GNSS tests verify the portable session contract through fake serial owners.
They do not verify HAL FIFO reset, physical pin routing, or UART behavior across sleep on hardware.
