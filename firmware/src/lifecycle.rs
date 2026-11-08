//! Runtime composition of service lifecycle handles.
//!
//! # Design choices
//!
//! - Keep the selected services outside the hardware suspend coordinator.
//! - Restore all present services after partial preparation so cancellation cannot strand a worker.
//!
//! See [worker coordination](../../docs/platform/worker-coordination.md#runtime-composition).

use core::convert::Infallible;
use paper_mesh_platform::{power_off::PowerOffParticipant, suspend::SuspendParticipant};

pub struct Services {
    pub clock: crate::rtc::FirmwareClock,
    pub usb: crate::usb::UsbLifecycle,
    pub button_polling: crate::buttons::ButtonPollingLifecycle,
    /// Absent when the shared bus or touch controller could not initialize.
    pub touch_input: Option<crate::touch::TouchInputLifecycle>,
    /// Absent when shared-bus initialization prevented the worker from starting.
    pub power_observation: Option<crate::power::PowerObservationLifecycle>,
}

impl Services {
    pub fn request_pause(&mut self) {
        self.clock.request_pause();
        self.usb.request_pause();
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.request_pause();
        }
        self.button_polling.request_pause();
        if let Some(observation) = &mut self.power_observation {
            observation.request_pause();
        }
    }
}

impl SuspendParticipant for Services {
    type Error = Infallible;

    async fn will_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.usb.will_suspend(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.will_suspend(deadline_ms).await?;
        }
        self.button_polling.will_suspend(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.will_suspend(deadline_ms).await?;
        }
        self.clock.will_suspend(deadline_ms).await
    }

    async fn did_resume(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.clock.did_resume(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.did_resume(deadline_ms).await?;
        }
        self.button_polling.did_resume(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.did_resume(deadline_ms).await?;
        }
        self.usb.did_resume(deadline_ms).await
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.clock.did_cancel_suspend(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.did_cancel_suspend(deadline_ms).await?;
        }
        self.button_polling.did_cancel_suspend(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.did_cancel_suspend(deadline_ms).await?;
        }
        self.usb.did_cancel_suspend(deadline_ms).await
    }
}

impl PowerOffParticipant for Services {
    type Error = Infallible;

    async fn will_power_off(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.usb.will_power_off(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.will_power_off(deadline_ms).await?;
        }
        self.button_polling.will_power_off(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.will_power_off(deadline_ms).await?;
        }
        self.clock.will_power_off(deadline_ms).await
    }
}
