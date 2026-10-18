# LILYGO T5 S3 E-Paper Pro hardware references

This index lists external references for the **H752-01 full Pro with LoRa and GPS**.
For driver implementation, first read the board schematic and pin map.
Vendor documents stay on upstream hosts.
This repository stores links and source metadata.
It does not store copies of documents, images, or drawings.

The [LILYGO backend profile](../../../boards/lilygo-t5-s3-pro.md) identifies each
control and signal family.
It maps them to [portable service contracts](../../../device-interfaces.md).
These sources establish facts about this backend.
They do not define requirements for all application targets.

| Reference | Upstream document |
| --- | --- |
| Board schematic, V1.0 dated 2024-12-24 | [T5 E-paper S3 Pro](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/T5%20E-paper%20S3%20Pro%20V1.0%2024-12-24.pdf) |
| LoRa board schematic, V1.0 dated 2024-04-20 | [LoRa-Top](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/LoRa-Top%20V1.0%2024-04-20.pdf) |
| Bus connections, peripheral addresses, power controls, and GPS variants | [Pin map](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pinmap.md) |
| GPIO and expander assignments | [Pin definitions](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pin_define.md) |
| Product variants and vendor setup guide | [English README](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/README.md), [Chinese README](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/README_CN.md) |
| Flash, PSRAM, and board build settings | [Board definition](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/boards/T5-ePaper-S3.json) |
| ED047TC1 e-paper panel | [Datasheet](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/ED047TC1.pdf) |
| TPS65185 display power supply | [Datasheet](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/tps65185.pdf) |
| PCA9535 I/O expander | [Datasheet](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/pca9535.pdf) |
| MIA-M10Q GPS option | [Datasheet](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/MIA-M10Q_DataSheet_UBX-22015849.pdf) |
| L76K GPS option | [Protocol specification](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/Quectel_L76K_GNSS_Protocol_Specification_V1.1.pdf) |
| Mechanical drawings | [Board size](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/DXF/H752-Board%20size.dxf), [shell size](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/DXF/H752-Shell%20size.dwg), [H752 drawing](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/DXF/H752.dxf) |

## Provenance

- Repository: [Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO).
- Branch: `H752-01`.
- Reviewed commit: [`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
- Retrieval date: 2026-10-12. The reviewed commit was the branch head on this date.
- Online product reference: [LILYGO wiki](https://wiki.lilygo.cc/products/t5-series/t5-e-paper-s3-pro/).

[SOURCES.json](SOURCES.json) records viewing and download URLs at specified
revisions, upstream paths, Git blob IDs, byte counts, and SHA-256 digests.
It also lists supplementary images and the vendor flashing guide.
Availability depends on the upstream host.
A fixed revision identifies the reviewed version but does not store it.

The inventory also uses these implementation references from the manifest:

| Evidence | Reference |
| --- | --- |
| Capacitive Home-key setup | [Factory initialization](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/main.cpp#L328-L351) |
| Home callback event semantics | [GT911 driver](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L203-L213) |
| Key register names and host coordinate handling | [GT911 constants](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/REG/GT911Constants.h), [coordinate helper](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvInterface.cpp#L77-L99) |
| Battery-only versus USB shutdown | [Charger example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq25896/main/main.ino) |
| Battery observations and units | [Fuel-gauge example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq27220/main/main.ino) |
| GNSS variant/baud probing | [GNSS example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino) |
| Optional touch/pen capabilities | [GT6972P example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/touch_gt6972p/README.md) |

The [TI BQ25896 datasheet, SLUSC76C, section 9.2.10](https://www.ti.com/lit/ds/symlink/bq25896.pdf#page=26)
gives information about QON, shipping mode, and reset behavior.
[Espressif ESP32-S3 sleep modes](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/sleep_modes.html)
gives information about wake-source restrictions.
Upstream hosts maintain these URLs without fixed Git revisions.
During a new review, check the cited revision, section, and platform.

The [Home-key investigation](../../../boards/lilygo-t5-s3-pro-touch.md#gt911-home-key-configuration-and-lower-bezel)
has a review date of 2026-10-13.
It uses Goodix's [GT911 Programming Guide, Rev.10, 2017-07-26](https://www.lcd-module.de/fileadmin/eng/pdf/zubehoer/GT911_Programming_Guide_Rev.10.pdf),
sections 3.2, 3.3, and 4.4, hosted by DISPLAY VISIONS.
It also uses the [GT911 datasheet, Rev.09](https://www.olimex.com/Products/OLinuXino/LCD/_resources/GT911_Datasheet.pdf),
section 5.4, hosted by Olimex.
These references give information about the controller.
They do not supply the fitted panel's electrode layout or factory configuration.

Keep these documents external.

Board and component documents cover the vendor's available variants.
Before use of another unit, confirm its fitted hardware.
Mechanical filenames keep the vendor's `H752` names on the `H752-01` branch.
The vendor's Arduino and PlatformIO instructions apply to its example software.
Use the [development guide](../../../development.md) for this project's development procedure.

## Retrieve a document for local inspection

Download documents into the devcontainer's temporary directory.
The example below retrieves the panel datasheet.
It checks the downloaded bytes against the recorded source metadata.

```sh
devcontainer exec --workspace-folder . python3 - <<'PYTHON'
import hashlib
import json
from pathlib import Path
from urllib.request import urlopen

sources = json.loads(Path('docs/vendor/lilygo/t5-s3-pro/SOURCES.json').read_text())
entry = next(item for item in sources['files']
             if item['upstream_path'] == 'hardware/ED047TC1.pdf')
with urlopen(entry['download_url'], timeout=30) as response:
    data = response.read()
assert len(data) == entry['bytes']
assert hashlib.sha256(data).hexdigest() == entry['sha256']
output = Path('/tmp/paper-mesh-vendor-docs/ED047TC1.pdf')
output.parent.mkdir(parents=True, exist_ok=True)
output.write_bytes(data)
print(output)
PYTHON
```

Do not commit downloaded documents, extracted text, or document screenshots.
Record findings in the project's own words in the [backend profile](../../../boards/lilygo-t5-s3-pro.md)
or [display evidence](../../../boards/lilygo-t5-s3-pro-display.md).
Include upstream links at specified revisions and applicable page or section
references. During a new source review, update affected URLs and metadata.
Update changed findings with the new evidence.
