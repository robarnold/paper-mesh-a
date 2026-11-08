//! USB transport state and ESP32-S3 hardware integration.
//!
//! # Design choices
//!
//! - Compile session and lifecycle tests without hardware so they test the owning crate's modules.
//!
//! See [USB adapter](../../../../docs/adapters/usb.md).

mod control;
mod lifecycle;

#[cfg(feature = "esp32s3")]
mod device;
#[cfg(feature = "esp32s3")]
pub use device::{
    CommandHandler, Config, DiagnosticSource, FirmwareTouchpad, REQUEST_CAPACITY, UsbControl,
    changed, is_session_active, send_control_reply, start,
};
#[cfg(feature = "esp32s3")]
use device::{RESTART, change};
pub use lifecycle::UsbLifecycle;
