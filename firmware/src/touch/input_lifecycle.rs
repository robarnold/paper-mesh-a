//! Lifecycle ownership for touch reports and HOME input.
//!
//! # Design choices
//!
//! - Await interrupt cleanup and queue clearing before the coordinator arms touch wake.
//! - Keep wake-source policy in the system coordinator.
//!
//! See [touch input coordination](../../../docs/platform/worker-coordination.md#touch-input-integration).

use core::convert::Infallible;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use paper_mesh_platform::{
    power_off::PowerOffParticipant, suspend::SuspendParticipant, worker::PauseControl,
};

pub struct TouchInputLifecycle {
    pub(super) pause: PauseControl<'static, CriticalSectionRawMutex>,
}

impl TouchInputLifecycle {
    /// Request a pause before reset without claiming the worker has stopped.
    pub fn request_pause(&mut self) {
        self.pause.request_pause();
    }
}

impl SuspendParticipant for TouchInputLifecycle {
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

impl PowerOffParticipant for TouchInputLifecycle {
    type Error = Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.pause().await;
        Ok(())
    }
}
