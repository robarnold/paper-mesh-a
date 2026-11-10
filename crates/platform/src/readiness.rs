//! Portable notification that device data may be ready to read.
//!
//! # Design choices
//!
//! - Keep event delivery separate from device report reads and executor placement.
//! - Keep captured events independent of delivery so acquisition control can suspend the reader without losing readiness.
//!
//! See [device readiness](../../../docs/platform/readiness.md).

use core::future::Future;

/// Observation time in microseconds on the runtime monotonic clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ready {
    pub at_us: u64,
}

/// A readiness notification means check the device, not that exactly one report exists.
/// Notifications may coalesce. A condition already active when waiting begins must not be missed.
/// Dropping a pending wait stops that delivery attempt but must retain captured readiness for a later wait.
/// Capture can continue without a waiting reader. The next wait must observe retained events or an active condition.
/// A readiness wait alone does not establish driver quiescence. Acquisition control and wake control are separate capabilities.
pub trait Readiness {
    fn wait(&mut self) -> impl Future<Output = Ready> + Send;
}
