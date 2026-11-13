//! Coordinate auxiliary-rail transitions with the consumers that depend on it.
//!
//! # Design choices
//!
//! - Prepare consumers before cutting their shared rail, and restore power before releasing admission.
//! - Treat every attempted power cut as uncertain until enable succeeds, including cancellation during I/O.
//!
//! See [auxiliary rail ownership](../../../../docs/boards/lilygo-t5-s3-pro-power.md#auxiliary-rail).

use core::{convert::Infallible, future::Future, num::NonZeroU64};
use paper_mesh_platform::{power_off::PowerOffParticipant, suspend::SuspendParticipant};

/// Physical rail operations used by firmware coordination.
pub trait Rail {
    type Error;
    /// Establish usable power before consumers resume. Permit retry after a failed or canceled transition.
    fn enable(&mut self) -> impl Future<Output = Result<(), Self::Error>>;
    /// Cut power only after consumers prepare. Failure or cancellation can leave the physical state uncertain.
    fn disable(&mut self) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Failure to prepare consumers or change their shared supply.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<C, R> {
    Consumer(C),
    Rail(R),
}
impl<C, R> From<Infallible> for Error<C, R> {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

/// Firmware-owned rail and its coordinated consumer lifecycle.
/// A consumer group can supply the same lifecycle interface when more devices become active.
pub struct Coordinator<R, C> {
    rail: R,
    /// Consumer handles retained for reset-recovery pause requests.
    pub(crate) consumers: C,
    needs_enable: bool,
    started: bool,
    next_startup_ms: u64,
    retry_ms: NonZeroU64,
}
impl<R: Rail, C: SuspendParticipant> Coordinator<R, C> {
    /// Consumers must start with acquisition admission blocked until this coordinator releases them.
    pub fn new(rail: R, consumers: C, retry_ms: NonZeroU64) -> Self {
        Self {
            rail,
            consumers,
            needs_enable: true,
            started: false,
            next_startup_ms: 0,
            retry_ms,
        }
    }

    /// Next retry deadline, or the maximum timestamp after startup completes.
    pub fn next_startup_ms(&self) -> u64 {
        if self.started {
            u64::MAX
        } else {
            self.next_startup_ms
        }
    }

    /// Attempt due startup without holding the application in a retry loop.
    /// Failure retains blocked consumer admission and schedules another attempt.
    pub async fn poll_startup(&mut self, now_ms: u64) -> Result<(), Error<C::Error, R::Error>> {
        if self.started || now_ms < self.next_startup_ms {
            return Ok(());
        }
        self.next_startup_ms = now_ms.saturating_add(self.retry_ms.get());
        self.did_resume(self.next_startup_ms).await
    }

    async fn restore_rail(&mut self) -> Result<(), Error<C::Error, R::Error>> {
        if self.needs_enable {
            self.rail.enable().await.map_err(Error::Rail)?;
        }
        Ok(())
    }
}
impl<R: Rail, C: SuspendParticipant> SuspendParticipant for Coordinator<R, C> {
    type Error = Error<C::Error, R::Error>;

    async fn will_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.consumers
            .will_suspend(deadline_ms)
            .await
            .map_err(Error::Consumer)?;
        // Record uncertainty before I/O so a dropped future cannot bypass restoration.
        self.needs_enable = true;
        self.rail.disable().await.map_err(Error::Rail)
    }

    async fn did_resume(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.restore_rail().await?;
        self.consumers
            .did_resume(deadline_ms)
            .await
            .map_err(Error::Consumer)?;
        self.needs_enable = false;
        self.started = true;
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.restore_rail().await?;
        self.consumers
            .did_cancel_suspend(deadline_ms)
            .await
            .map_err(Error::Consumer)?;
        self.needs_enable = false;
        self.started = true;
        Ok(())
    }
}
impl<R: Rail, C: PowerOffParticipant> PowerOffParticipant for Coordinator<R, C> {
    type Error = Error<C::Error, R::Error>;

    async fn will_power_off(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.consumers
            .will_power_off(deadline_ms)
            .await
            .map_err(Error::Consumer)?;
        self.needs_enable = true;
        self.rail.disable().await.map_err(Error::Rail)
    }
}

#[cfg(test)]
mod tests;
