//! Firmware features that connect adapter capabilities to runtime policy.
//!
//! # Design choices
//!
//! - Keep each integration in its own module so firmware can compose features independently.
//!
//! See [runtime ownership](../../docs/architecture.md).

pub mod touch_wake;
