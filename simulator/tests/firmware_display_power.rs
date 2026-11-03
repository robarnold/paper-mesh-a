//! Panel supply sequencing tests.
//!
//! # Design choices
//!
//! - Run the production panel power sequence tests on the host to check ordering
//!   under injected faults.
//!
//! See [Power sequence and recovery](../../docs/boards/lilygo-t5-s3-pro-display.md#power-sequence-and-recovery).

#[path = "../../firmware/src/display/power_sequence.rs"]
mod power_sequence;
