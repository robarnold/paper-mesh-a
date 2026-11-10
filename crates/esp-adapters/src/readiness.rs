//! GPIO readiness notifications and separate wake control.
//!
//! # Design choices
//!
//! - Keep GPIO acknowledgement and rearming private to the provider.
//! - Retain wake capture independently of the hardware latch and ordinary readiness waits.
//!
//! See [GPIO readiness provider](../../../docs/adapters/gpio-readiness.md).

#[cfg(feature = "esp32s3")]
mod gpio;
mod state;
#[cfg(feature = "esp32s3")]
pub use gpio::{GpioReadiness, Signal, WakeControl};

/// Reason that a wake interval could not start. Refusal leaves provider state unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmError {
    /// Capture has not started, or a wake interval is already armed.
    NotCapturing,
    /// An ordinary readiness wait still owns delivery.
    ReaderActive,
    /// The input already has its configured active level.
    InputActive,
}

/// Capture result for a completed wake interval. This is an observation, not an operation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WakeCapture {
    NotCaptured,
    Captured,
}

/// Input evidence used to restore a consumer after suspend or canceled preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestorationObservation {
    /// A wake event retained since the last arm or restoration observation.
    pub wake: WakeCapture,
    /// Whether the pin currently has its configured active level.
    pub input_active: bool,
}
