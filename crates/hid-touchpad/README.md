# HID touchpad protocol

This `no_std` crate builds a five-contact HID touchpad descriptor and encodes input reports.
It depends on platform contact types, with no application or USB controller dependency.
Backends supply logical geometry and physical dimensions.

The [USB backend profile](../../docs/boards/lilygo-t5-s3-pro-usb.md) defines current transport settings and limits.
The [host contract](../../docs/services/touchpad.md) defines session cancellation and input ownership.

The descriptor follows the USB HID Digitizers usages and native touchpad input mode.
See the [HID Usage Tables](https://usb.org/document-library/hid-usage-tables-16).
Linux interpretation is defined in [`hid-multitouch.c`](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-multitouch.c).
The certification feature has an empty payload. It does not establish Windows certification or compatibility.

Native tests cover release positions, complete replacement frames, feature requests, and scan timestamps.
The `descriptor` example writes the prototype report descriptor to standard output for host parser checks.
