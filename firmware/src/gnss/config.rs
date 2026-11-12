//! Runtime choices for receiver acquisition and recovery.
//!
//! # Design choices
//!
//! - Keep baud candidates and timing choices outside sentence decoding and UART execution.
//!
//! See [GNSS configuration](../../../docs/boards/lilygo-t5-s3-pro-gnss.md#configuration).

use core::num::{NonZeroU32, NonZeroU64};

/// Nonempty, ordered baud candidates. The first rate is required even when no other rates are supplied.
#[derive(Clone, Copy)]
pub struct BaudRates {
    pub first: NonZeroU32,
    pub remaining: &'static [NonZeroU32],
}

impl BaudRates {
    /// Read a candidate by its position in the first-then-remaining sequence.
    pub(super) fn at(self, index: usize) -> NonZeroU32 {
        if index == 0 {
            self.first
        } else {
            self.remaining[index - 1]
        }
    }
}

/// Baud candidates and nonzero durations supplied by firmware for one receiver worker.
#[derive(Clone, Copy)]
pub struct Config {
    /// Candidates tried in order. The type requires at least one nonzero rate.
    pub baud_rates: BaudRates,
    /// Milliseconds allowed for each candidate before valid input establishes reception.
    pub probe_ms: NonZeroU64,
    /// Milliseconds of silence allowed after a valid sentence before baud probing resumes.
    pub silence_ms: NonZeroU64,
    /// Milliseconds to wait after supply enable or UART construction fails.
    pub retry_ms: NonZeroU64,
}
