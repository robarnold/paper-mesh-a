//! GNSS preparation results and worker lifecycle ownership.
//!
//! # Design choices
//!
//! - Read the preparation result only after pause acknowledgement so stale results cannot approve shutdown.
//! - Keep a failed preparation paused until suspend cancellation or power-off reset recovery.
//!
//! See [GNSS coordination](../../../docs/platform/worker-coordination.md#gnss-integration).

use core::{cell::Cell, convert::Infallible};
use embassy_sync::blocking_mutex::Mutex;
#[cfg(not(test))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as PauseMutex;
#[cfg(test)]
use embassy_sync::blocking_mutex::raw::NoopRawMutex as PauseMutex;
use paper_mesh_platform::{
    power_off::PowerOffParticipant, suspend::SuspendParticipant, worker::PauseControl,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationError {
    pub idle_uart_failed: bool,
    pub supply_shutdown_failed: bool,
}

impl From<Infallible> for PreparationError {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

pub(super) type PreparationReport = Mutex<PauseMutex, Cell<Result<(), PreparationError>>>;

pub(super) fn publish_preparation(
    report: &PreparationReport,
    idle_uart_ready: bool,
    supply_disabled: bool,
) {
    let result = if idle_uart_ready && supply_disabled {
        Ok(())
    } else {
        Err(PreparationError {
            idle_uart_failed: !idle_uart_ready,
            supply_shutdown_failed: !supply_disabled,
        })
    };
    report.lock(|state| state.set(result));
}

pub struct GnssLifecycle {
    pub(super) pause: PauseControl<'static, PauseMutex>,
    pub(super) report: &'static PreparationReport,
}

impl GnssLifecycle {
    /// Request a pause before reset without claiming preparation succeeded.
    pub fn request_pause(&mut self) {
        self.pause.request_pause();
    }

    async fn prepare(&mut self) -> Result<(), PreparationError> {
        self.pause.pause().await;
        self.report.lock(Cell::get)
    }
}

impl SuspendParticipant for GnssLifecycle {
    type Error = PreparationError;

    async fn will_suspend(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.prepare().await
    }

    async fn did_resume(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.resume();
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.did_resume(deadline_ms).await
    }
}

impl PowerOffParticipant for GnssLifecycle {
    type Error = PreparationError;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.prepare().await
    }
}

#[cfg(test)]
#[path = "lifecycle/tests.rs"]
mod tests;
