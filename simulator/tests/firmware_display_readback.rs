//! Framebuffer and refresh record readback tests.
//!
//! # Design choices
//!
//! - Run the firmware's frame readback logic on the host to check retained
//!   diagnostics without driving a panel.
//!
//! See [Inspect the held framebuffer](../../docs/boards/lilygo-t5-s3-pro-usb.md#inspect-the-held-framebuffer).

extern crate alloc;

#[allow(dead_code)]
#[path = "../../firmware/src/display/power_sequence.rs"]
mod power_sequence;

#[allow(dead_code)]
#[path = "../../firmware/src/display/inspection.rs"]
mod inspection;
