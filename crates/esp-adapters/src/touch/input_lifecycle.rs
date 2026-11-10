//! Lifecycle ownership for touch reports and HOME input.
//!
//! # Design choices
//!
//! - Quiesce acquisition before clearing queued input and inspecting contact state.
//! - Restore logical input before releasing acquisition delivery.
//! - Keep wake-source policy in the system coordinator.
//!
//! See [touch input coordination](../../../../docs/platform/worker-coordination.md#touch-input-integration).

use core::convert::Infallible;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use paper_mesh_platform::{
    acquisition::Control, power_off::PowerOffParticipant, suspend::SuspendParticipant,
};

/// Coordinate acquisition quiescence with logical touch preparation and restoration.
pub struct TouchInputLifecycle {
    /// Block new acquisitions and await release of the device before logical input changes.
    pub(super) control: Control<'static, CriticalSectionRawMutex>,
    /// Device and contact state, accessible here only after acquisition acknowledges quiescence.
    pub(super) state: &'static super::InputState,
    /// Logical preparation completed. False also covers cancellation while acquisition is still in flight.
    pub(super) prepared: bool,
}

impl TouchInputLifecycle {
    /// Request a pause before reset without claiming the worker has stopped.
    pub fn request_pause(&mut self) {
        self.control.request_quiescence();
    }
}

impl SuspendParticipant for TouchInputLifecycle {
    type Error = Infallible;

    async fn will_suspend(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.control.quiesce().await;
        self.state.prepare_suspend();
        self.prepared = true;
        Ok(())
    }

    async fn did_resume(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        // Canceled preparation can leave a request active without reaching quiescence.
        // Release that request, but do not access device state that the worker may still own.
        if self.prepared {
            self.state.restore();
            self.prepared = false;
        }
        self.control.resume();
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.did_resume(deadline_ms).await
    }
}

impl PowerOffParticipant for TouchInputLifecycle {
    type Error = Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.control.quiesce().await;
        self.state.prepare_suspend();
        self.prepared = true;
        Ok(())
    }
}
