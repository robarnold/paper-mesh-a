//! Observation timing independent of serial configuration.
//!
//! See [GNSS configuration](../../../docs/boards/lilygo-t5-s3-pro-gnss.md#configuration).

use core::num::NonZeroU64;

/// Maximum silence before current receiver observations become unavailable.
#[derive(Clone, Copy)]
pub struct Config {
    pub silence_ms: NonZeroU64,
}
