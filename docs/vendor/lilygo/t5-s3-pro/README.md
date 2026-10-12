# LILYGO T5 S3 E-Paper Pro hardware references

Official documentation snapshot for the **H752-01 full Pro with LoRa and GPS**.
Start with the board schematic and pin map below when implementing drivers.

| Reference | Local copy |
| --- | --- |
| Board schematic, V1.0 dated 2024-12-24 | [T5 E-paper S3 Pro](<upstream/hardware/T5 E-paper S3 Pro V1.0 24-12-24.pdf>) |
| LoRa board schematic, V1.0 dated 2024-04-20 | [LoRa-Top](<upstream/hardware/LoRa-Top V1.0 24-04-20.pdf>) |
| Bus connections, peripheral addresses, power controls, and GPS variants | [Pin map](upstream/docs/pinmap.md) |
| GPIO and expander assignments | [Pin definitions](upstream/docs/pin_define.md) |
| Product variants and vendor setup guide | [English README](upstream/README.md), [Chinese README](upstream/README_CN.md) |
| Flash, PSRAM, and board build settings | [Board definition](upstream/boards/T5-ePaper-S3.json) |
| ED047TC1 e-paper panel | [Datasheet](upstream/hardware/ED047TC1.pdf) |
| TPS65185 display power supply | [Datasheet](upstream/hardware/tps65185.pdf) |
| PCA9535 I/O expander | [Datasheet](upstream/hardware/pca9535.pdf) |
| MIA-M10Q GPS option | [Datasheet](upstream/hardware/MIA-M10Q_DataSheet_UBX-22015849.pdf) |
| L76K GPS option | [Protocol specification](upstream/hardware/Quectel_L76K_GNSS_Protocol_Specification_V1.1.pdf) |
| Mechanical drawings | [Board size](<upstream/DXF/H752-Board size.dxf>), [shell size](<upstream/DXF/H752-Shell size.dwg>), [H752 drawing](upstream/DXF/H752.dxf) |

The snapshot also includes the vendor's documentation images and flashing guide,
so their relative links remain usable offline. Use this project's root README
and AGENTS.md for our development workflow; the vendor's Arduino/PlatformIO
instructions describe its own example software.

## Provenance

- Repository: [Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO).
- Branch: `H752-01`.
- Commit: [`938d6189db0fde9a12558629bc1405bb49e1be78`](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/tree/938d6189db0fde9a12558629bc1405bb49e1be78).
- Retrieved: 2026-10-12; this was the branch head at retrieval.
- Online product reference: [LILYGO wiki](https://wiki.lilygo.cc/products/t5-series/t5-e-paper-s3-pro/).

Files under `upstream/` are copied byte for byte, preserving filenames, notices,
formatting, and relative directory structure. [SOURCES.json](SOURCES.json)
records each file's upstream URL, Git blob ID, size, and SHA-256 digest;
[SHA256SUMS](SHA256SUMS) provides an offline integrity check. Firmware binaries,
example code, libraries, and the historical schematic are excluded.

The board and component documents cover the vendor's available variants; confirm
the fitted hardware when the unit arrives. The mechanical filenames retain the
vendor's `H752` naming even though they are published on the `H752-01` branch.

The upstream repository has no root license file at this revision. Existing
document copyright notices are retained, and this snapshot does not assign a
new license to vendor or component-manufacturer material.

## Verify or refresh

Run the integrity check inside the devcontainer from the repository root:

```sh
devcontainer exec --workspace-folder . bash -lc \
  'cd docs/vendor/lilygo/t5-s3-pro && sha256sum --check SHA256SUMS'
```

For a refresh, compare a specific upstream commit with this recorded revision,
copy only the relevant documentation, and regenerate both manifests. Keep the
upstream files unchanged; put project-specific interpretation in our own docs.
