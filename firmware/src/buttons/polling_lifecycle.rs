//! Lifecycle ownership for BOOT and Function polling.
//!
//! # Design choices
//!
//! - Await event publication and bus release before the coordinator arms wake sources.
//! - Keep polling separate from HOME input and hardware wake interrupts.
//!
//! See [button polling coordination](../../../docs/platform/worker-coordination.md#button-polling-integration).

use core::convert::Infallible;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use paper_mesh_platform::{
    power_off::PowerOffParticipant, suspend::SuspendParticipant, worker::PauseControl,
};

pub struct ButtonPollingLifecycle {
    pub(super) pause: PauseControl<'static, CriticalSectionRawMutex>,
}

impl ButtonPollingLifecycle {
    /// Request a pause before reset without claiming the worker has stopped.
    pub fn request_pause(&mut self) {
        self.pause.request_pause();
    }
}

impl SuspendParticipant for ButtonPollingLifecycle {
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

impl PowerOffParticipant for ButtonPollingLifecycle {
    type Error = Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.pause.pause().await;
        Ok(())
    }
}
