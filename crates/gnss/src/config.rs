//! Observation timing independent of serial configuration.
//!
//! See [GNSS acquisition](../../../docs/adapters/gnss.md#acquisition).

use core::num::NonZeroU64;

/// Maximum silence before current receiver observations become unavailable.
#[derive(Clone, Copy)]
pub struct Config {
    pub silence_ms: NonZeroU64,
}
