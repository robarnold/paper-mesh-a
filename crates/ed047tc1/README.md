# ED047TC1 scan encoding

This crate stores packed Gray4 pixels in native 960 × 540 coordinates.
The firmware supplies two framebuffers in PSRAM, with 518,400 bytes total.
One framebuffer holds the new image. The other holds the last completed image.
The crate does not allocate memory or control hardware.

The scan sequence and phase times derive from
[`lilygo-t5s3paperpro-rs`](https://github.com/azw413/lilygo-t5s3paperpro-rs/tree/c9cf0be45a33c4b2574aa3dde6dc128bb7ed7233),
by Andrew Whaley and Frido Koch.
That project derives from `fridolin-koch/lilygo-epd47-rs`.
This crate and `firmware/src/display.rs` use GPL-3.0-only.
The [license](LICENSE-GPL-3.0) applies to these driver sources.
This notice does not change the license of unrelated project files.

The driver uses a full refresh at startup or after changes to gray pixels.
A full refresh clears the panel, then applies the reference's fifteen black-on-white phases.
Black-to-white and white-to-black changes use five partial phases, with 100 µs drive time per phase.
Unchanged pixels receive neutral commands, including unchanged gray pixels.
An unchanged image requires no panel update.
The driver saves the completed image only after the scans and supply shutdown succeed.
It does not use calibrated, temperature-dependent waveform tables.
Optical shade accuracy needs physical measurement.
