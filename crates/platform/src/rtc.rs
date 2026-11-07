//! Calendar access and synchronization independent of the processor and RTC model.
//!
//! # Design choices
//!
//! - Separate retained calendar operations from cached `Clock` reads so hardware
//!   failures remain available to synchronization code.
//! - Accept synchronization policy from the caller instead of selecting board limits.
//!
//! See the [time contract](../../../docs/services/time.md).

use crate::time::UtcDateTime;

pub mod synchronization;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidTimeReason {
    /// The device reports voltage loss that invalidates retained time.
    PowerLost,
    ClockStopped,
    TestMode,
    /// The stored calendar convention is outside the adapter's supported range.
    UnsupportedCalendar,
    InvalidCalendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtcError<E> {
    Access(E),
    InvalidTime(InvalidTimeReason),
    UnsupportedDate,
    VerificationFailed,
}

/// Retained calendar access. Operations can perform I/O and partially change hardware.
pub trait Rtc {
    type AccessError;

    fn read_utc(&mut self) -> Result<UtcDateTime, RtcError<Self::AccessError>>;

    /// Write calendar time and verify the result through device readback.
    /// Adapters document their tolerance for elapsed time during verification.
    /// Success does not establish retention across power loss or oscillator accuracy.
    fn set_utc(&mut self, utc: UtcDateTime) -> Result<(), RtcError<Self::AccessError>>;
}
