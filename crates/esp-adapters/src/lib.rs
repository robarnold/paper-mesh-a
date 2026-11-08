#![no_std]

//! ESP hardware adapters for platform contracts.
//!
//! # Design choices
//!
//! - Select hardware dependencies explicitly so native tests can exercise portable adapter state.
//! - Receive device configuration and protocol policy from the runtime.
//!
//! See [adapter boundaries](../README.md).

extern crate alloc;
#[cfg(test)]
extern crate std;

#[cfg(any(feature = "esp32s3", test))]
pub mod usb;
