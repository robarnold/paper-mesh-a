# LILYGO display backend evidence

This is evidence for the [LILYGO backend](lilygo-t5-s3-pro.md), informing the
[portable display contract](../display-constraints.md). Its numbers and driver
algorithms are not universal application requirements.

Evidence reviewed on **2026-10-12**. This describes the ED047TC1 panel and
reference driver approaches; we have not implemented or benchmarked our physical
Rust driver. Numerical examples below are calculations, not measured device
performance. The simulator's 250 ms refresh is a test delay.

## Panel facts and terminology

The [ED047TC1 specification hosted by LILYGO](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/ED047TC1.pdf)
identifies 540 gate lines and 960 source positions in its scan-direction drawing
(p. 8). We represent each gate line as a **960-pixel native row**, giving native
storage of 960 × 540. The mechanical specification describes the same panel in
portrait as 540 × 960 (p. 4). These are different coordinate conventions for the
same pixels, not different panels.

The interface has eight source-data signals plus source/gate clocks and control
signals (pp. 6–7). The specification lists a maximum refresh rate of **85 Hz**
(p. 11), AC timing constraints (pp. 12–13), an operating range of **0–50 °C**
(p. 10), and waveform/controller-dependent grayscale capability (p. 4).
Electrical and optical guarantees are conditional on the manufacturer's
controller and waveforms (p. 11).

Here, a **scan** is one traversal applying drive commands; a **transition** is
the multi-scan process moving pixels toward their target shades. An application's
new image need not finish in one scan. The 85 Hz specification therefore does
not promise 85 completed grayscale images per second.

The host driver implements waveform scheduling over the panel's source/gate
drivers. Its two-bit drive commands are not two-bit image colors: grayscale
comes from a sequence of drives. Modos's controller documentation explains this
distinction and the role of previous pixel state, waveform phase, and temperature
in generating those commands. [Source: Glider/Caster design][caster]

## What determines update cost?

There are several independent costs; the count of newly changed pixels is not
enough to predict latency:

| Work | Useful inputs to measure | When a small change helps |
| --- | --- | --- |
| Layout and rasterization | Invalidated components, pixels drawn | Components preserve unaffected content and avoid unnecessary re-layout |
| Difference detection | Pixels/bytes compared, candidate regions | Valid damage bounds avoid a full-frame comparison |
| Transition processing | Active transitions, waveform mode, phase/history | Driver omits settled regions; pixels still completing earlier changes remain active |
| Scan output | Native rows visited, full rows transferred, rows skipped | Backend actually shortens inactive-row handling |
| Physical transition | Drive duration, source/target shade, temperature | Smaller area does not by itself shorten the required drive for each pixel |
| Power and cleanup | Rail startup, discharge, cleanup cycles | Updates are batched and the chosen mode permits reuse of a known panel state |

The gate driver advances through lines; it is not a random-access pixel store.
In the reference implementations below, an active row carries a full row of
drive commands, including neutral commands at unchanged positions. Row skipping
can advance inactive lines faster, but still has timing and pipeline overhead.
An update rectangle does not automatically imply a proportional transfer saving.

### Row skipping is backend-specific

The following are source-code observations at pinned revisions, not claims
about every driver bearing the same name:

| Reference | What the inspected implementation does | Consequence |
| --- | --- | --- |
| [EPDiy I2S output][epdiy-i2s], `i2s_skip_row` | Flushes the row pipeline, then advances inactive rows with gate pulses | Long inactive runs can reduce scan time; skipping is not free |
| [EPDiy LCD output][epdiy-lcd], `lcd_calculate_frame` | Queues neutral line buffers for inactive rows; the [LCD engine][epdiy-engine] still traverses the configured display lines | Avoided waveform work does not imply avoided full scan output |
| [FastEPD][fastepd], `bbepPartialUpdate` | The inspected 1-bpp path iterates all native rows and writes a full-width neutral row outside the active line range | Partial/differential updates need not transfer fewer rows |

EPDiy exposes dirty-line and cropped-difference information, while its API also
distinguishes fast monochrome and grayscale waveform modes. Availability depends
on the loaded waveform; merely defining an enum value does not make that mode
supported. [Source: EPDiy API][epdiy-api]

Cheaper handling of unchanged rows therefore needs a capability check: it holds
for a backend with accelerated skipping, not automatically for the ESP32-S3 LCD
path or FastEPD. These projects are references, not dependencies of this Rust
application.

### Rotation changes the shape of the work

Damage must be transformed into **native coordinates before estimating scan
cost**. Under our current quarter-turn, one native row appears as a vertical
column in the portrait UI. A portrait rectangle's width becomes its native row
count; its height becomes its span within each row.

For example, two completely changing rectangles contain the same 8,000 pixels:

| Portrait rectangle | Native rectangle | Native rows touched | Active-row payload per scan, if rows can be skipped |
| --- | --- | --- | --- |
| 40 × 200 | 200 × 40 | 40 | 9,600 bytes |
| 200 × 40 | 40 × 200 | 200 | 48,000 bytes |

These calculations assume 960 positions per transmitted row and two drive bits
per position. They exclude inactive-row overhead and waveform dwell time. A
full-scan backend can transmit **129,600 bytes for either case**. The fivefold
active-row payload difference is not a promise of fivefold lower latency.
Scattered changes can also cover many more native rows than one compact region.

This is an argument for measuring damage shape and driver capabilities, not for
forcing every component into a particular orientation. Usability still decides
layout; the display service can report the cost of that layout's update pattern.

### Throughput and memory examples

For 960 × 540, a full scan contains 518,400 pixel positions. At a hypothetical
60 scans/second, that is **31.104 million positions/second** and **7.776 MB/s** of
two-bit drive payload, before blanking and protocol overhead. Rasterization,
frame comparisons, waveform lookup, state writes, and buffer copies consume
additional CPU time and memory bandwidth.

A packed 4-bpp target image is 259,200 bytes; old and new images together need
518,400 bytes before driver state or DMA buffers. Our byte-per-pixel simulator
uses 518,400 bytes per image and is not the hardware memory layout. Available
PSRAM capacity alone says nothing about whether scan deadlines can be met.

Modos Caster supports independently timed pixels and early redirection of
transitions. Its documented 16-bit per-pixel state is read and written every
scan, contributing to a 4.5-byte-per-position memory-traffic example including
the target image. This removes a global update barrier at a substantial memory
cost; it does not make pigment motion instantaneous. [Source: Caster design][caster]

Wenting Zhang's PaperBoy report demonstrates the technique on ESP32-S3 using
480 × 144 independently processed positions at 60 Hz, SRAM-resident buffers,
LCD/DMA output, and roughly 50% CPU time for that processing. It does **not**
establish full-resolution performance on our board. [Source: author's report][paperboy]

## Mapping evidence to the display contract

The [portable display contract](../display-constraints.md) captures damage,
quality intent, capabilities, completion, and recovery. The driver implements
native scan scheduling, waveform history, temperature handling, and power
sequencing. Components do not inspect panel IDs or choose row-skipping code.

The row-cost examples motivate a backend cost estimate, not a universal cost
formula or mandatory UI orientation. Buffer sizes belong to this profile; the
application receives logical geometry and supported rendering capabilities.

FastEPD's release notes describe charge imbalance and residual gray from repeated
unbalanced updates. This motivates cleanup capabilities and measured policy,
not a universal refresh interval. [Source: FastEPD release notes][fastepd-releases]

## Simulator and hardware validation

The next simulator model should record logical damage, native rows covered,
actual rows transferred/skipped, pending transitions, update mode, and queue age.
Give it separate full-scan and row-skipping profiles. Label timing estimates as
synthetic until fitted to measurements; do not invent a universal pixels-to-ms
conversion. Test equal-area shapes, scattered changes, rotated layouts, rapid
input during refresh, errors, and idle periods without display work.

When hardware is available, measure submission-to-first-visible-change and
submission-to-settled-image separately from CPU/render time and scan duration.
Compare black/white and grayscale transitions, compact versus scattered damage,
both orientations, warm versus cold power rails, and temperature. Record driver
revision, waveform, clock settings, CPU/PSRAM placement, row counts, cleanup
history, and energy per update. A logic analyzer can establish electrical
timing; optical observation is needed for settling and ghosting. No hardware
rate, grayscale-quality target, cleanup interval, or battery budget is claimed yet.

[caster]: https://github.com/Modos-Labs/Glider/blob/ed94ef7fd95a42cfdcdaf5e5da1dcdd709122763/README.md#low-latency-drive
[epdiy-i2s]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_i2s/render_i2s.c#L71
[epdiy-lcd]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_lcd/render_lcd.c#L192
[epdiy-engine]: https://github.com/vroland/epdiy/blob/42c16127dc5d4bca15e830624f22666ad914575f/src/output_lcd/lcd_driver.c#L147
[epdiy-api]: https://epdiy.readthedocs.io/en/latest/api.html
[fastepd]: https://github.com/bitbank2/FastEPD/blob/a5aa810798c5104c1da8dc915f75ab00a4b65f13/src/FastEPD.inl#L3478
[fastepd-releases]: https://github.com/bitbank2/FastEPD/releases/tag/1.5.1
[paperboy]: https://www.hackster.io/wenting-zhang/60fps-eink-gameboy-emulator-on-m5papers3-57e4e5
