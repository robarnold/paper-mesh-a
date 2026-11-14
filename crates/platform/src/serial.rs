//! Exclusive serial receive sessions with fresh input boundaries.
//!
//! # Design choices
//!
//! - Reuse embedded I/O for reads without requiring lossless cancellation when a session ends.
//! - Keep pins, executor placement, and sleep preparation in the backend.
//!
//! See [serial sessions](../../../docs/platform/serial.md).

use core::num::NonZeroU32;
pub use embedded_io_async::{Error, ErrorKind, ErrorType, Read};

/// Serial resource owner that lends one receive session at a time.
pub trait SerialReceiver {
    type OpenError;
    type Session<'a>: Read
    where
        Self: 'a;

    /// Configure fresh reception, discarding input and errors from earlier sessions.
    /// Failure leaves no session and permits retry.
    /// Dropping a session releases reception resources, but does not establish system sleep readiness.
    /// A pending read may lose bytes on cancellation. End the session before reading again after cancellation.
    fn open(&mut self, baud: NonZeroU32) -> Result<Self::Session<'_>, Self::OpenError>;
}
