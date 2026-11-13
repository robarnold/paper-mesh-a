//! Board auxiliary rail shared by the GNSS receiver and LoRa chip.
//!
//! # Design choices
//!
//! - Own expander access separately from consumers because disabling this rail affects both devices.
//! - Hold the shared bus for each complete register sequence to preserve unrelated expander outputs.
//!
//! See [auxiliary rail ownership](../../docs/boards/lilygo-t5-s3-pro-power.md#auxiliary-rail).

mod device;

use crate::bus::Bus;
use esp_hal::i2c::master::Error;

/// Firmware control of the physical auxiliary rail, including both connected devices.
pub struct AuxiliaryRail {
    bus: &'static Bus,
}

impl AuxiliaryRail {
    /// Connect the rail to the initialized bus without changing its output state.
    pub fn new(bus: &'static Bus) -> Self {
        Self { bus }
    }

    /// Enable the rail, preserving unrelated outputs. A failed write can leave partial hardware state.
    pub async fn enable(&mut self) -> Result<(), Error> {
        device::enable(&mut *self.bus.lock().await)
    }

    /// Disable power to every consumer. Firmware must first coordinate all users of this rail.
    pub async fn disable(&mut self) -> Result<(), Error> {
        device::disable(&mut *self.bus.lock().await)
    }
}

impl crate::features::auxiliary_power::Rail for AuxiliaryRail {
    type Error = Error;

    async fn enable(&mut self) -> Result<(), Error> {
        AuxiliaryRail::enable(self).await
    }

    async fn disable(&mut self) -> Result<(), Error> {
        AuxiliaryRail::disable(self).await
    }
}
