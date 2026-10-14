# LILYGO display backend evidence

This evidence for the [LILYGO backend](lilygo-t5-s3-pro.md) supports the
[portable display contract](../display-constraints.md).
Its numerical examples and driver algorithms apply to this backend.
They are not universal application requirements.

The evidence review date is **2026-10-12**.
The review covers the ED047TC1 panel and reference drivers.
The project has no physical Rust display driver or hardware benchmark yet.
The examples below are calculations, not measured performance.
The simulator's 250 ms refresh is an artificial test delay.

## Panel facts and terminology

The [ED047TC1 specification hosted by LILYGO](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/ED047TC1.pdf)
shows 540 gate lines and 960 source positions in the scan-direction drawing
(page 8).
This project represents each gate line as a **960-pixel native row**.
Native storage is therefore 960 × 540.
The mechanical specification gives information about the same panel as 540 × 960 in portrait
(page 4). These are coordinate conventions for the same pixels.

The interface has eight source-data signals, source and gate clocks, and control
signals (pages 6–7).
The specification lists a maximum refresh rate of **85 Hz** (page 11).
It specifies AC timing limits (pages 12–13) and an operating range of **0–50 °C**
(page 10).
Grayscale capability depends on the waveform and controller (page 4).
Electrical and optical guarantees depend on the manufacturer's controller and
waveforms (page 11).

A **scan** is one traversal that applies drive commands.
A **transition** is a sequence of scans that moves pixels toward their target
shades. A new application image can take multiple scans.
Thus, 85 Hz does not guarantee 85 completed grayscale images per second.

The host driver schedules waveforms through the panel's source and gate drivers.
Two-bit drive commands do not represent two-bit image colors.
Grayscale comes from a sequence of drives.
The Modos controller documentation gives information about last pixel state, waveform
phase, and temperature as inputs to command generation.
[Source: Glider/Caster design][caster]

## What determines update cost?

Update latency depends on multiple independent costs.
The number of changed pixels alone is insufficient.

| Work | Useful measurements | Conditions that reduce work |
| --- | --- | --- |
| Layout and rasterization | Invalidated components, pixels drawn | Components keep unaffected content and avoid unnecessary layout changes. |
| Difference detection | Compared pixels and bytes, candidate regions | Valid damage bounds prevent a full-frame comparison. |
| Transition processing | Active transitions, waveform mode, phase, history | The driver omits settled regions. Incomplete earlier transitions stay active. |
| Scan output | Visited native rows, transferred rows, skipped rows | The backend shortens work for inactive rows. |
| Physical transition | Drive duration, source shade, target shade, temperature | Smaller area alone does not shorten the required drive for each pixel. |
| Power and cleanup | Rail startup, discharge, cleanup cycles | Combined updates and the selected mode permit reuse of known panel state. |

The gate driver advances through lines.
It does not provide random pixel access.
In these reference drivers, an active row carries a complete row of drive
commands. Unchanged positions receive neutral commands.
Row skipping can advance inactive lines faster, but has timing and pipeline
costs. An update rectangle does not guarantee proportional transfer savings.

### Row skipping is backend-specific

These observations apply to inspected code at specified revisions.
Other implementations with the same name can differ.

| Reference | Inspected behavior | Consequence |
| --- | --- | --- |
| [EPDiy I2S output][epdiy-i2s], `i2s_skip_row` | Flushes the row pipeline, then advances inactive rows with gate pulses | Long inactive runs can reduce scan time. Skipping has a cost. |
| [EPDiy LCD output][epdiy-lcd], `lcd_calculate_frame` | Queues neutral buffers for inactive rows. The [LCD engine][epdiy-engine] traverses the configured display lines. | Reduced waveform work does not imply reduced scan output. |
| [FastEPD][fastepd], `bbepPartialUpdate` | The inspected 1-bpp path visits all native rows. It writes full-width neutral rows outside the active line range. | Partial or differential updates can transfer the same number of rows. |

EPDiy supplies dirty-line and cropped-difference information.
Its API separates fast monochrome modes from grayscale modes.
Mode availability depends on the loaded waveform.
An enum value alone does not establish support.
[Source: EPDiy API][epdiy-api]

Reduced cost for unchanged rows depends on backend capabilities.
Accelerated row skipping can supply this reduction.
The ESP32-S3 LCD path and FastEPD do not automatically supply it.
These projects are references, not Rust application dependencies.

### Rotation changes the shape of the work

Scan-cost calculations must use damage in **native coordinates**.
Under the current quarter-turn, one native row appears as a vertical portrait
column. A portrait rectangle's width becomes its native row count.
Its height becomes its span in each row.

Two fully changed rectangles can each contain 8,000 pixels:

| Portrait rectangle | Native rectangle | Native rows touched | Active-row payload per scan, if rows can be skipped |
| --- | --- | --- | --- |
| 40 × 200 | 200 × 40 | 40 | 9,600 bytes |
| 200 × 40 | 40 × 200 | 200 | 48,000 bytes |

These calculations assume 960 positions per transmitted row and two drive bits
per position. They exclude inactive-row costs and waveform dwell time.
A full-scan backend can transmit **129,600 bytes for either case**.
The fivefold payload difference does not guarantee a fivefold latency reduction.
Scattered changes can cover more native rows than one compact region.

These examples show why measurements of damage shape and driver capabilities are necessary.
They do not specify a fixed orientation for every component.
Usability determines layout.
The display service can report the cost of the resulting update pattern.

### Throughput and memory examples

For 960 × 540, a full scan contains 518,400 pixel positions.
A hypothetical rate of 60 scans/second gives **31.104 million positions/second**.
The two-bit drive payload is **7.776 MB/s**, before blanking and protocol costs.
Rasterization, frame comparisons, waveform lookup, state writes, and buffer
copies use more CPU time and memory bandwidth.

A packed 4-bpp target image uses 259,200 bytes.
Old and new images together use 518,400 bytes before driver state or DMA
buffers. The simulator uses one byte per pixel, or 518,400 bytes per image.
That format does not define hardware storage.
Available PSRAM capacity alone cannot establish whether scan deadlines are
possible.

Modos Caster supports independent pixel timing and early transition redirection.
Its documented state uses 16 bits for each pixel.
Each scan reads and writes that state.

With the target image, the documented example uses 4.5 bytes of memory traffic
per position.
This approach removes a global update barrier at substantial memory cost.
It does not make pigment motion instantaneous.
[Source: Caster design][caster]

Wenting Zhang's PaperBoy report demonstrates this technique on ESP32-S3.
It processes 480 × 144 positions independently at 60 Hz.
It uses SRAM buffers and LCD/DMA output.
The report gives approximately 50% CPU time for that processing.
It does not establish full-resolution performance on this board.
[Source: author's report][paperboy]

## Mapping evidence to the display contract

The [portable display contract](../display-constraints.md) defines damage,
quality intent, capabilities, completion, and recovery.
The driver controls native scans, waveform history, temperature handling, and
power sequences. Components do not inspect panel IDs or select row-skipping code.

Row costs support backend estimates.
They do not define a universal formula or mandatory UI orientation.
Buffer sizes belong to this profile.
The application receives logical geometry and rendering capabilities.

FastEPD release notes give information about charge imbalance and residual gray after repeated
unbalanced updates. These effects support cleanup capabilities and policy based
on measurements. They do not establish a universal refresh interval.
[Source: FastEPD release notes][fastepd-releases]

## Simulator and hardware validation

The next simulator model will record logical damage, native row coverage, transferred
and skipped rows, pending transitions, update mode, and queue age.
Separate profiles will represent full scans and row skipping.
Timing estimates will stay synthetic until calibration against measurements.
The model must not assume a universal pixel-count-to-time conversion.

Scenarios must include equal-area shapes, scattered changes, rotated layouts, rapid
input during refresh, errors, and idle periods without display work.

Hardware measurements will separate submission-to-first-change time,
submission-to-settled-image time, CPU time, render time, and scan duration.
Comparisons will cover black/white and grayscale transitions, compact and
scattered damage, both orientations, rail startup state, and temperature.
Records will include driver revision, waveform, clocks, CPU and PSRAM placement,
row counts, cleanup history, and energy per update.

A logic analyzer can establish electrical timing.
Optical observations are necessary for settling and ghosting measurements.
No hardware rate, grayscale quality target, cleanup interval, or battery budget
is established yet.

[caster]: https://github.com/Modos-Labs/Glider/blob/ed94ef7fd95a42cfdcdaf5e5da1dcdd709122763/README.md#low-latency-drive
[epdiy-i2s]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_i2s/render_i2s.c#L71
[epdiy-lcd]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_lcd/render_lcd.c#L192
[epdiy-engine]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_lcd/lcd_driver.c#L147
[epdiy-api]: https://epdiy.readthedocs.io/en/latest/api.html
[fastepd]: https://github.com/bitbank2/FastEPD/blob/a5aa810798c5104c1da8dc915f75ab00a4b65f13/src/FastEPD.inl#L3478
[fastepd-releases]: https://github.com/bitbank2/FastEPD/releases/tag/1.5.1
[paperboy]: https://www.hackster.io/wenting-zhang/60fps-eink-gameboy-emulator-on-m5papers3-57e4e5
