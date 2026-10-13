# LILYGO T5 S3 E-Paper Pro hardware references

External documentation for the **H752-01 full Pro with LoRa and GPS**.
Start with the board schematic and pin map below when implementing drivers.
Vendor documents remain hosted upstream; this repository contains links and
source metadata, not copies of the documents, images, or drawings.
Our [LILYGO backend profile](../../../boards/lilygo-t5-s3-pro.md) accounts for
every control and signal family and maps them to the
[portable service contracts](../../../device-interfaces.md). These sources
establish facts about this backend, not requirements for all application targets.

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
- Retrieved: 2026-10-12; this was the branch head at retrieval.
- Online product reference: [LILYGO wiki](https://wiki.lilygo.cc/products/t5-series/t5-e-paper-s3-pro/).

[SOURCES.json](SOURCES.json) records pinned viewing and download URLs, upstream
paths, Git blob IDs, byte counts, and SHA-256 digests. It also indexes the
vendor's supplementary images and flashing guide. Availability depends on the
upstream host; pinning identifies the reviewed version but does not preserve it.

The inventory also uses pinned implementation references indexed in the manifest:

| Evidence | Reference |
| --- | --- |
| Capacitive Home-key setup | [Factory initialization](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/factory/main/main.cpp#L328-L351) |
| Home callback event semantics | [GT911 driver](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvGT911.hpp#L203-L213) |
| Key register names and host coordinate handling | [GT911 constants](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/REG/GT911Constants.h), [coordinate helper](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/SensorLib/src/TouchDrvInterface.cpp#L77-L99) |
| Battery-only versus USB shutdown | [Charger example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq25896/main/main.ino) |
| Battery observations and units | [Fuel-gauge example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/bq27220/main/main.ino) |
| GNSS variant/baud probing | [GNSS example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino) |
| Optional touch/pen capabilities | [GT6972P example](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/touch_gt6972p/README.md) |

Manufacturer references: [TI BQ25896 datasheet, SLUSC76C, section 9.2.10](https://www.ti.com/lit/ds/symlink/bq25896.pdf#page=26)
for QON/shipping/reset behavior, and [Espressif ESP32-S3 sleep modes](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/sleep_modes.html)
for wake-source restrictions. These URLs are maintained upstream rather than
pinned Git files; the cited revision/section or platform matters when rechecking.

For the [Home-key investigation](../../../boards/lilygo-t5-s3-pro.md#gt911-home-key-configuration-and-lower-bezel),
reviewed on 2026-10-13, use Goodix's [GT911 Programming Guide, Rev.10, 2017-07-26](https://www.lcd-module.de/fileadmin/eng/pdf/zubehoer/GT911_Programming_Guide_Rev.10.pdf)
(hosted by DISPLAY VISIONS, sections 3.2, 3.3, and 4.4) and [GT911 datasheet, Rev.09](https://www.olimex.com/Products/OLinuXino/LCD/_resources/GT911_Datasheet.pdf)
(hosted by Olimex, section 5.4). These are controller references, not the fitted
LILYGO touch-panel electrode layout or a dump of its factory configuration.
Keep these documents external as well.

The board and component documents cover the vendor's available variants; confirm
the fitted hardware when the unit arrives. The mechanical filenames retain the
vendor's `H752` naming even though they are published on the `H752-01` branch.
Use this project's root README and AGENTS.md for our development workflow; the
vendor's Arduino/PlatformIO instructions describe its own example software.

## Retrieve a document for local inspection

Download inside the devcontainer into its temporary directory. For example,
retrieve and verify the panel datasheet against the recorded source:

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
Record relevant findings in our own words in the [backend profile](../../../boards/lilygo-t5-s3-pro.md)
or [display evidence](../../../boards/lilygo-t5-s3-pro-display.md), citing the pinned
upstream document and relevant pages or sections. When reviewing a newer source,
update the affected URLs and metadata along with any changed findings.
