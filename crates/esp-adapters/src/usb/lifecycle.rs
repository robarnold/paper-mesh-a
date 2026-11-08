//! USB lifecycle ownership across endpoint workers and the device worker.
//!
//! # Design choices
//!
//! - Stop endpoint operations before device detachment so canceled writes cannot use disabled endpoints.
//! - Keep pause requests active until restoration or reset, including after canceled preparation.
//!
//! See [worker coordination](../../../../docs/platform/worker-coordination.md#usb-integration).

#[cfg(not(test))]
use super::{RESTART, change};
use core::convert::Infallible;
#[cfg(not(test))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as PauseMutex;
#[cfg(test)]
use embassy_sync::blocking_mutex::raw::NoopRawMutex as PauseMutex;
use paper_mesh_platform::{
    power_off::PowerOffParticipant,
    suspend::SuspendParticipant,
    worker::{PauseControl, PauseWorker},
};
#[cfg(test)]
use tests::{RESTART, change};

pub(super) type Worker = PauseWorker<'static, PauseMutex>;
type Control = PauseControl<'static, PauseMutex>;

pub struct UsbLifecycle {
    pub(super) device: Control,
    pub(super) hid: Control,
    pub(super) logs: Control,
    pub(super) commands: Control,
}

impl UsbLifecycle {
    /// Request all workers to stop before reset, without claiming safe detachment.
    pub fn request_pause(&mut self) {
        self.hid.request_pause();
        self.logs.request_pause();
        self.commands.request_pause();
        self.device.request_pause();
    }

    async fn prepare(&mut self) {
        self.hid.request_pause();
        self.logs.request_pause();
        self.commands.request_pause();
        self.commands.pause().await;
        self.hid.pause().await;
        self.logs.pause().await;
        self.device.pause().await;
    }

    fn restore(&mut self) {
        // Invalidate sessions even when preparation stopped before device detachment.
        change(0);
        RESTART.signal(());
        self.device.resume();
        self.hid.resume();
        self.logs.resume();
        self.commands.resume();
    }
}

impl SuspendParticipant for UsbLifecycle {
    type Error = Infallible;

    async fn will_suspend(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.prepare().await;
        Ok(())
    }

    async fn did_resume(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.restore();
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.did_resume(deadline_ms).await
    }
}

impl PowerOffParticipant for UsbLifecycle {
    type Error = Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.prepare().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
