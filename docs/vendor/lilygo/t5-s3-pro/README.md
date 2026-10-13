# LILYGO T5 S3 E-Paper Pro hardware references

External documentation for the **H752-01 full Pro with LoRa and GPS**.
Start with the board schematic and pin map below when implementing drivers.
Vendor documents remain hosted upstream; this repository contains links and
source metadata, not copies of the documents, images, or drawings.

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
Record relevant findings in our own words in [hardware.md](../../../hardware.md)
or [display-constraints.md](../../../display-constraints.md), citing the pinned
upstream document and relevant pages or sections. When reviewing a newer source,
update the affected URLs and metadata along with any changed findings.
