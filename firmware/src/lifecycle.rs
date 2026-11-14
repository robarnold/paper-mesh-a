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
    crate::gnss_worker::GnssLifecycle,
>;
type Error = crate::features::auxiliary_power::Error<
    crate::gnss_worker::PreparationError,
    esp_hal::i2c::master::Error,
>;

/// Runtime-owned lifecycle ordering for services and shared power dependencies.
pub struct Services {
    /// Own consumer ordering for the shared auxiliary rail.
    pub auxiliary: AuxiliaryPower,
    pub clock: paper_mesh_esp_adapters::rtc::RtcClock,
    pub usb: crate::usb::UsbLifecycle,
    pub button_polling: crate::buttons::ButtonPollingLifecycle,
    /// Absent when the shared bus or touch controller could not initialize.
    pub touch_input: Option<paper_mesh_esp_adapters::touch::TouchInputLifecycle>,
    /// Absent when shared-bus initialization prevented the worker from starting.
    pub power_observation: Option<crate::power::observation::PowerObservationLifecycle>,
}

impl Services {
    /// Establish the shared supply before application work. Failure requires a fresh hardware initialization.
    pub async fn start(&mut self) {
        use embassy_time::{Duration, Instant, with_timeout};
        const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
        let deadline_ms = (Instant::now() + STARTUP_TIMEOUT).as_millis();
        match with_timeout(STARTUP_TIMEOUT, self.auxiliary.did_resume(deadline_ms)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::error!("auxiliary rail: startup failed: {:?}; resetting", error);
                esp_hal::system::software_reset();
            }
            Err(_) => {
                log::error!("auxiliary rail: startup timed out; resetting");
                esp_hal::system::software_reset();
            }
        }
    }

    pub fn request_pause(&mut self) {
        self.auxiliary.consumers.request_pause();
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
        self.auxiliary.will_suspend(deadline_ms).await?;
        Ok(())
    }

    async fn did_resume(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.auxiliary.did_resume(deadline_ms).await?;
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
        self.auxiliary.did_cancel_suspend(deadline_ms).await?;
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
        self.auxiliary.will_power_off(deadline_ms).await?;
        Ok(())
    }
}
