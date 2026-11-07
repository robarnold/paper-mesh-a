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
    InvalidCalendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtcError<E> {
    Access(E),
    InvalidTime(InvalidTimeReason),
    UnsupportedDate,
}

/// Retained calendar access. Operations can perform I/O and partially change hardware.
pub trait Rtc {
    type AccessError;

    fn read_utc(&mut self) -> Result<UtcDateTime, RtcError<Self::AccessError>>;

    /// Write calendar time. The caller reads again to check calendar validity.
    /// Success does not establish retention or equality with the requested time.
    fn set_utc(&mut self, utc: UtcDateTime) -> Result<(), RtcError<Self::AccessError>>;
}
