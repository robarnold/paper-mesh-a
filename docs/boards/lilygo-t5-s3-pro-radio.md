# LILYGO radio discovery

[Backend profile](lilygo-t5-s3-pro.md)

`firmware/src/discovery/` implements the shared `Discovery` interface.
One Embassy worker owns `WIFI` and `BT` on core 0.
It initializes only the radio for the active request.
It destroys that controller before it delivers results or starts another scan.
Cancellation invalidates observations immediately and wakes the worker for resource release.
The request deadline includes initialization, the scan, and resource release.

The firmware reserves 128 KiB of internal RAM for radio drivers and RTOS tasks.
Radio futures also use this heap to limit temporary stack storage.
Application results and worker results use separate static buffers, each with 24 entries.
The application does not allocate heap memory.

The CPU operates at 240 MHz for display scans.
Display storage uses PSRAM. Core 1 executes display refreshes.

| Radio | Scan configuration | Result conversion |
| --- | --- | --- |
| Wi-Fi | Passive scan, 120 ms per channel, hidden SSIDs included, default driver country configuration | BSSID, SSID, signed RSSI. The adapter requests up to 25 records and retains 24. An extra record sets the result-limit indication. |
| Bluetooth LE | Passive legacy scan, 1M PHY, 100 ms interval and window, four-second observation period, duplicate filtering disabled | Public or random address, advertised local name, signed RSSI. RSSI 127 means unavailable. Addresses use display byte order. |

The Wi-Fi driver supplies its records in signal order.
The shared result buffer applies the final order and address tie-break.
The Wi-Fi API exposes only the valid UTF-8 prefix of an SSID.
The adapter replaces unavailable suffix bytes with question marks.
An empty SSID stays a hidden network.

Bluetooth reports update RSSI for the same address and address type.
An omitted name preserves a previous name from the same scan.
A complete local name takes precedence over a shortened name within one advertisement.
Malformed advertising data produces an unnamed observation.
Malformed HCI reports produce a scan failure and discard partial results.
The adapter does not resolve private addresses into stable identities.

Passive scans do not send probe requests or scan requests.
Thus, a device name that appears only in a scan response can stay unavailable.
The Bluetooth adapter does not request extended or coded-PHY advertisements.
Wi-Fi discovery covers the ESP32-S3's 2.4 GHz radio.
These scans do not establish a connection, pair a device, or transmit application data.
LoRa stays uninitialized.

The implementation pins `esp-radio` to `1.0.0-beta.1` and uses `bt-hci` 0.9.
The lockfile fixes the resolved dependencies.
These upstream sources define the adapter assumptions:

| Source | Applicable sections |
| --- | --- |
| [esp-radio Wi-Fi source][radio-wifi] | `WifiController::new`, `scan_async`, `WifiRefGuard::drop`, `wifi_deinit`, and `Ssid::as_str` |
| [esp-radio scan source][radio-scan] | `ScanConfig`, `ScanTypeConfig::Passive`, and `ScanResults` |
| [esp-radio Bluetooth source][radio-ble] | `BleConnector::new`, `Drop`, and the HCI transport implementation |
| [esp-radio runtime source][radio-runtime] | Allocator requirement, `init`, and minimum CPU frequency |
| [bt-hci controller source][hci-controller] | Concurrent command acknowledgment and event reception in `ExternalController` |
| [bt-hci command source][hci-commands] | `LeSetScanParams`, `LeSetScanEnable`, and Bluetooth specification references |
| [Espressif Wi-Fi reference][wifi-records] | `esp_wifi_scan_get_ap_records` signal order and scan record lifecycle |

The pinned Wi-Fi wrapper does not expose `ScanDone.status` to its caller.
The adapter reports errors that the wrapper returns.
It cannot distinguish every failure inside the driver from an empty result.
Controller destruction also handles shutdown errors inside the driver.
Physical tests must therefore check repeated scans, cancellation, and radio changes for resource leaks or continued radio activity.
The compile checks and native tests do not establish physical operation or heap capacity under radio traffic.

## Diagnostic build

The default firmware waits for application requests.
The display and touch adapters connect the radio debugger to the physical screen.
The optional `radio-scan-demo` feature performs one Wi-Fi scan and one Bluetooth LE scan at boot.
It prints the result order, addresses, names, RSSI, errors, and result-limit indication through USB Serial/JTAG.

To build this diagnostic firmware without device access, run:

```sh
devcontainer exec --workspace-folder . bash -lc 'cd firmware && cargo build --locked --release --features radio-scan-demo'
```

This command writes the ELF under `firmware/target/xtensa-esp32s3-none-elf/release/`.
`just ci` checks this feature, then produces the default firmware image.
Neither command flashes a device.

[radio-wifi]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/wifi/mod.rs
[radio-scan]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/wifi/scan.rs
[radio-ble]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/ble/controller/mod.rs
[radio-runtime]: https://github.com/esp-rs/esp-hal/blob/f769fcd27056b0837febe4209f3bae78ed764561/esp-radio/src/lib.rs
[hci-controller]: https://github.com/embassy-rs/bt-hci/blob/d0f7558ff205313c2c473046adec0ffdc00d919b/src/controller.rs
[hci-commands]: https://github.com/embassy-rs/bt-hci/blob/d0f7558ff205313c2c473046adec0ffdc00d919b/src/cmd/le.rs
[wifi-records]: https://docs.espressif.com/projects/esp-idf/en/v5.2.6/esp32s3/api-reference/network/esp_wifi.html#_CPPv428esp_wifi_scan_get_ap_recordsP8uint16_tP16wifi_ap_record_t
