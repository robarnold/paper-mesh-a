//! Service preparation and failures from an explicit system power-off request.
//!
//! # Design choices
//!
//! - Keep power-off participation independent of suspend because persistence and resource requirements can differ.
//! - Distinguish external power from preparation failures so the caller can tell the
//!   user when to disconnect a supply before retrying.
//! - Report continued power after a shutdown attempt so the caller can distinguish a
//!   request from physical power removal.
//!
//! See the [power-off contract](../../../docs/services/power.md#explicit-power-off).

/// A service control handle for graceful power removal, independent of suspend support.
/// The coordinator serializes calls, controls work admission, and orders dependencies.
/// It enforces deadlines in the monotonic `Clock::now_ms` epoch.
pub trait PowerOffParticipant {
    type Error;

    /// Finish required persistence and release resources needed for power removal.
    /// Success keeps the service prepared. An error prevents graceful power removal.
    /// Dropping this future can leave partial preparation. Aborted shutdown requires backend reset recovery.
    fn will_power_off(
        &mut self,
        deadline_ms: u64,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerOffError {
    Unsupported,
    Busy,
    ExternalPower,
    Preparation,
    StillPowered,
}

impl PowerOffError {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unsupported => "Power off is unavailable",
            Self::Busy => "Device busy. Try again",
            Self::ExternalPower => "Disconnect USB and external power",
            Self::Preparation => "Power-off preparation failed",
            Self::StillPowered => "Device still powered. Try again",
        }
    }
}
