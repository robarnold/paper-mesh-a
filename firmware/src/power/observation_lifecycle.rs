//! Lifecycle ownership for periodic power observations.
//!
//! # Design choices
//!
//! - Await completed observations and bus release before acknowledging preparation.
//! - Keep hardware power removal in the system coordinator, separate from observation workers.
//!
//! See [power worker coordination](../../../docs/platform/worker-coordination.md#power-observation-integration).

use core::convert::Infallible;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use paper_mesh_platform::{
    power_off::PowerOffParticipant, suspend::SuspendParticipant, worker::PauseControl,
};

pub struct PowerObservationLifecycle {
    pub(super) pause: PauseControl<'static, CriticalSectionRawMutex>,
}

impl PowerObservationLifecycle {
    /// Request a pause before reset without claiming the worker has stopped.
    pub fn request_pause(&mut self) {
        self.pause.request_pause();
    }
}

impl SuspendParticipant for PowerObservationLifecycle {
    type Error = Infallible;

    async fn will_suspend(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.pause().await;
        Ok(())
    }

    async fn did_resume(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.resume();
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.did_resume(deadline_ms).await
    }
}

impl PowerOffParticipant for PowerObservationLifecycle {
    type Error = Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.pause().await;
        Ok(())
    }
}
