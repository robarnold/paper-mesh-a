//! Board power control and separate observation access.
//!
//! # Design choices
//!
//! - Keep power-off admission and battery isolation separate from the observation worker.
//! - Retain the board shutdown sequence in the system coordinator.
//!
//! See [system power-off](../../../docs/boards/lilygo-t5-s3-pro-power.md#system-power-off).

mod device;
pub mod observation;
pub(crate) mod ship;

use crate::bus::Bus;

pub(crate) fn ship_error(
    error: ship::Error<esp_hal::i2c::master::Error>,
) -> paper_mesh_platform::power_off::PowerOffError {
    match error {
        ship::Error::ExternalPower => paper_mesh_platform::power_off::PowerOffError::ExternalPower,
        ship::Error::Bus(_) | ship::Error::Verification => {
            paper_mesh_platform::power_off::PowerOffError::Preparation
        }
    }
}

pub async fn check_power_off(
    bus: &Bus,
) -> Result<(), paper_mesh_platform::power_off::PowerOffError> {
    ship::check(&mut *bus.lock().await).map_err(ship_error)
}
