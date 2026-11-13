//! Native harness for the firmware auxiliary rail's register tests.
//!
//! See [auxiliary rail ownership](../../docs/boards/lilygo-t5-s3-pro-power.md#auxiliary-rail).

#[path = "../../firmware/src/auxiliary_rail/device/mod.rs"]
mod device;

#[path = "../../firmware/src/features/auxiliary_power/mod.rs"]
mod coordination;
