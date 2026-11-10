//! Board connection between touch wake admission and the GPIO wake capability.
//!
//! # Design choices
//!
//! - Keep the readiness worker separate from system sleep configuration.
//! - Retain captured GPIO wake state until touch restoration has inspected it.
//!
//! See [touch readiness and suspend](../../../docs/adapters/touch.md#readiness-and-suspend).

use core::cell::RefCell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use paper_mesh_esp_adapters::{
    readiness::{ArmError, WakeCapture, WakeControl},
    touch,
};
use paper_mesh_platform::suspend::SuspendError;

static WAKE: Mutex<CriticalSectionRawMutex, RefCell<Option<WakeControl>>> =
    Mutex::new(RefCell::new(None));

pub fn initialize(wake: WakeControl) {
    WAKE.lock(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(wake);
    });
}
pub fn arm() -> Result<(), SuspendError> {
    WAKE.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let wake = slot.as_mut().ok_or(SuspendError::Unsupported)?;
        if !touch::wake_ready() {
            return Err(SuspendError::WakeActive);
        }
        wake.arm_wake_capture().map_err(|error| match error {
            ArmError::InputActive => SuspendError::WakeActive,
            ArmError::NotCapturing | ArmError::ReaderActive => SuspendError::Preparation,
        })
    })
}
/// Finish an armed interval. Return whether the coordinator must record a touch wake event.
pub fn finish() -> bool {
    WAKE.lock(|slot| {
        slot.borrow_mut()
            .as_mut()
            .and_then(WakeControl::finish_wake_capture)
            == Some(WakeCapture::Captured)
    })
}
/// Consume restoration evidence. Return whether touch must suppress an uncertain contact sequence.
pub fn cancel_after_resume() -> bool {
    WAKE.lock(|slot| {
        slot.borrow_mut().as_mut().is_some_and(|wake| {
            let observation = wake.take_restoration_observation();
            observation.wake == WakeCapture::Captured || observation.input_active
        })
    })
}
