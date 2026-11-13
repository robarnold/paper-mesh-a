//! Runtime composition of service lifecycle handles.
//!
//! # Design choices
//!
//! - Keep the selected services outside the hardware suspend coordinator.
//! - Restore all present services after partial preparation so cancellation cannot strand a worker.
//!
//! See [worker coordination](../../docs/platform/worker-coordination.md#runtime-composition).

use paper_mesh_platform::{power_off::PowerOffParticipant, suspend::SuspendParticipant};

type AuxiliaryPower = crate::features::auxiliary_power::Coordinator<
    crate::auxiliary_rail::AuxiliaryRail,
    crate::gnss::GnssLifecycle,
>;
type Error = crate::features::auxiliary_power::Error<
    crate::gnss::PreparationError,
    esp_hal::i2c::master::Error,
>;

/// Runtime-owned lifecycle ordering for services and shared power dependencies.
pub struct Services {
    /// Absent when shared-bus initialization prevented receiver startup.
    pub auxiliary: Option<AuxiliaryPower>,
    pub clock: paper_mesh_esp_adapters::rtc::RtcClock,
    pub usb: crate::usb::UsbLifecycle,
    pub button_polling: crate::buttons::ButtonPollingLifecycle,
    /// Absent when the shared bus or touch controller could not initialize.
    pub touch_input: Option<paper_mesh_esp_adapters::touch::TouchInputLifecycle>,
    /// Absent when shared-bus initialization prevented the worker from starting.
    pub power_observation: Option<crate::power::observation::PowerObservationLifecycle>,
}

impl Services {
    pub async fn poll_startup(&mut self, now_ms: u64) {
        if let Some(auxiliary) = &mut self.auxiliary
            && let Err(error) = auxiliary.poll_startup(now_ms).await
        {
            log::error!("auxiliary rail: startup failed: {:?}", error);
            crate::gnss::fault();
        }
    }

    pub fn next_startup_ms(&self) -> u64 {
        self.auxiliary
            .as_ref()
            .map_or(u64::MAX, AuxiliaryPower::next_startup_ms)
    }

    pub fn request_pause(&mut self) {
        if let Some(auxiliary) = &mut self.auxiliary {
            auxiliary.consumers.request_pause();
        }
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
    type Error = Error;

    async fn will_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.usb.will_suspend(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.will_suspend(deadline_ms).await?;
        }
        self.button_polling.will_suspend(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.will_suspend(deadline_ms).await?;
        }
        self.clock.will_suspend(deadline_ms).await?;
        if let Some(auxiliary) = &mut self.auxiliary {
            auxiliary.will_suspend(deadline_ms).await?;
        }
        Ok(())
    }

    async fn did_resume(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        if let Some(auxiliary) = &mut self.auxiliary {
            auxiliary.did_resume(deadline_ms).await?;
        }
        self.clock.did_resume(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.did_resume(deadline_ms).await?;
        }
        self.button_polling.did_resume(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.did_resume(deadline_ms).await?;
        }
        self.usb.did_resume(deadline_ms).await?;
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        if let Some(auxiliary) = &mut self.auxiliary {
            auxiliary.did_cancel_suspend(deadline_ms).await?;
        }
        self.clock.did_cancel_suspend(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.did_cancel_suspend(deadline_ms).await?;
        }
        self.button_polling.did_cancel_suspend(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.did_cancel_suspend(deadline_ms).await?;
        }
        self.usb.did_cancel_suspend(deadline_ms).await?;
        Ok(())
    }
}

impl PowerOffParticipant for Services {
    type Error = Error;

    async fn will_power_off(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.usb.will_power_off(deadline_ms).await?;
        if let Some(touch_input) = &mut self.touch_input {
            touch_input.will_power_off(deadline_ms).await?;
        }
        self.button_polling.will_power_off(deadline_ms).await?;
        if let Some(observation) = &mut self.power_observation {
            observation.will_power_off(deadline_ms).await?;
        }
        self.clock.will_power_off(deadline_ms).await?;
        if let Some(auxiliary) = &mut self.auxiliary {
            auxiliary.will_power_off(deadline_ms).await?;
        }
        Ok(())
    }
}
