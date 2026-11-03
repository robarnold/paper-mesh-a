//! Test the production watchdog and retained-state rules on the host.
//!
//! # Design choices
//!
//! - Exercise watchdog ownership and retained checkpoints without requiring a
//!   physical reset.
//!
//! See [Power sequence and recovery](../../docs/boards/lilygo-t5-s3-pro-display.md#power-sequence-and-recovery).

#[allow(dead_code)]
#[path = "../../firmware/src/display_recovery/state.rs"]
mod display;
#[allow(dead_code)]
#[path = "../../firmware/src/recovery/state.rs"]
mod watchdog;
