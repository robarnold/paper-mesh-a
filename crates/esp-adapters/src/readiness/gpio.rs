//! ESP GPIO binding for the readiness provider.
//!
//! # Design choices
//!
//! - Accept a configured pin and active level from firmware without knowing the connected device.
//! - Share pin ownership internally while exposing separate readiness and wake handles.
//!
//! See [GPIO readiness provider](../../../../docs/adapters/gpio-readiness.md).

use super::state::{Input, State};
use super::{ArmError, RestorationObservation, WakeCapture};
use crate::interrupt::{GpioInterruptClient, GpioInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::Instant;
use esp_hal::gpio::{Event, Flex, Level};
use paper_mesh_platform::readiness::{Readiness, Ready};

struct Pin {
    pin: Flex<'static>,
    active: Level,
}
impl Input for Pin {
    fn active(&self) -> bool {
        self.pin.is_low() == (self.active == Level::Low)
    }
    fn pending(&self) -> bool {
        self.pin.is_interrupt_set()
    }
    fn mask(&mut self) {
        self.pin.unlisten();
    }
    fn clear(&mut self) {
        self.pin.clear_interrupt();
    }
    fn enable(&mut self) {
        self.pin.listen(if self.active == Level::Low {
            Event::LowLevel
        } else {
            Event::HighLevel
        });
    }
}

/// Own one GPIO pin across ordinary capture and separately armed wake intervals.
pub struct GpioReadiness {
    state: State<CriticalSectionRawMutex, Pin>,
    did_wake: fn(),
}
impl GpioReadiness {
    /// The caller supplies an input-enabled pin with its output driver disabled.
    /// `did_wake` runs in interrupt context and must not wait or access devices.
    pub fn new(pin: Flex<'static>, active: Level, did_wake: fn()) -> Self {
        Self {
            state: State::new(Pin { pin, active }),
            did_wake,
        }
    }
    /// Split once before registration and task launch. The provider owns all subsequent pin operations.
    pub fn split(&'static mut self) -> (Signal, WakeControl, GpioInterruptHandler) {
        (
            Signal { provider: self },
            WakeControl { provider: self },
            GpioInterruptHandler::for_client(self),
        )
    }
}
impl GpioInterruptClient for GpioReadiness {
    fn handle_interrupt(&self) {
        if self.state.did_interrupt(Instant::now().as_micros()) {
            (self.did_wake)();
        }
    }
}

/// Unique ordinary reader capability. Capture survives cancellation of its waits.
pub struct Signal {
    provider: &'static GpioReadiness,
}
impl Signal {
    /// Enable event capture after the runtime has registered and enabled interrupt delivery.
    pub fn start_capture(&mut self) {
        self.provider
            .state
            .start_capture(Instant::now().as_micros());
    }
}
impl Readiness for Signal {
    async fn wait(&mut self) -> Ready {
        self.provider
            .state
            .wait_for_readiness(|| Instant::now().as_micros())
            .await
    }
}

/// Separate capability for the sleep coordinator. Arm only after the readiness consumer has paused.
pub struct WakeControl {
    provider: &'static GpioReadiness,
}
impl WakeControl {
    /// Start a wake interval after acquisition quiescence. Refusal leaves capture unchanged.
    pub fn arm_wake_capture(&mut self) -> Result<(), ArmError> {
        self.provider
            .state
            .arm_wake_capture(Instant::now().as_micros())
    }
    /// Finish an armed interval and return its capture result. None means no interval was armed.
    /// Retain readiness and wake evidence for restoration. This operation does not fail.
    pub fn finish_wake_capture(&mut self) -> Option<WakeCapture> {
        self.provider
            .state
            .finish_wake_capture(Instant::now().as_micros())
    }
    /// Consume wake evidence and sample the current input level after finishing the wake interval.
    /// This does not consume ordinary readiness or release acquisition admission.
    pub fn take_restoration_observation(&mut self) -> RestorationObservation {
        self.provider.state.take_restoration_observation()
    }
}
