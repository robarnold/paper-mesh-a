//! Failures from an explicit system power-off request.
//!
//! # Design choices
//!
//! - Distinguish external power from preparation failures so the caller can tell the
//!   user when to disconnect a supply before retrying.
//! - Report continued power after a shutdown attempt so the caller can distinguish a
//!   request from physical power removal.
//!
//! See the [power-off contract](../../../docs/services/power.md#explicit-power-off).

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
