//! BQ27220 battery observations through embedded-hal I2C.
//!
//! # Design choices
//!
//! - Check presence and readiness before and after measurements to reject a changed sample.
//! - Read existing gauge estimates without changing configuration or claiming verified calibration.
//!
//! See [BQ27220 observations](../../../docs/adapters/bq27220.md).

#![no_std]

use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_hal::i2c::I2c;
use paper_mesh_platform::bus::Arbitrated;
use paper_mesh_platform::power::{BatteryState, GaugeState};

// The BQ27220 fixes its seven-bit I2C address at 0x55, independent of board wiring.
// The bus supplies the read/write bit. See datasheet section 7.3.1.1 in the adapter documentation.
const GAUGE: u8 = 0x55;

/// Battery observations with exclusive bus ownership for each complete sample.
pub struct Bq27220<'a, M: RawMutex, I, const N: usize> {
    bus: &'a Arbitrated<M, I, N>,
}

impl<'a, M: RawMutex, I, const N: usize> Bq27220<'a, M, I, N> {
    /// Use the supplied bus access without changing device configuration.
    pub const fn new(bus: &'a Arbitrated<M, I, N>) -> Self {
        Self { bus }
    }
}

impl<M: RawMutex, I: I2c, const N: usize> paper_mesh_platform::power::BatteryGauge
    for Bq27220<'_, M, I, N>
{
    type Error = Error<I::Error>;

    async fn read(&mut self) -> Result<BatteryState, Self::Error> {
        let mut bus = self.bus.lock().await;
        battery(&mut *bus)
    }
}

/// Failed bus access or a change in battery presence or gauge readiness during measurement.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    ChangedDuringRead,
}

fn word<I: I2c>(bus: &mut I, register: u8) -> Result<u16, Error<I::Error>> {
    let mut bytes = [0; 2];
    bus.write_read(GAUGE, &[register], &mut bytes)
        .map_err(Error::Bus)?;
    Ok(u16::from_le_bytes(bytes))
}

/// Read battery status and available measurements without changing gauge configuration.
/// The caller must hold the bus for the complete sample. Failure discards partial measurements.
pub fn battery<I: I2c>(bus: &mut I) -> Result<BatteryState, Error<I::Error>> {
    let status = word(bus, 0x0a)?;
    let operation = word(bus, 0x3a)?;
    let gauge = if operation & 0x0400 != 0 {
        GaugeState::Configuring
    } else if operation & 0x01 != 0 {
        GaugeState::Calibrating
    } else if operation & 0x20 == 0 {
        GaugeState::Initializing
    } else {
        GaugeState::Ready
    };
    let mut state = BatteryState {
        present: status & 0x08 != 0,
        gauge,
        fully_charged: status & 0x0200 != 0,
        fully_discharged: status & 0x8000 != 0,
        discharge_active: status & 0x01 != 0,
        charge_inhibited: status & 0x0100 != 0,
        overtemperature: status & 0x0c00 != 0,
        shutdown_requested: status & 0x02 != 0,
        ..BatteryState::default()
    };
    // A ready gauge does not prove correct cell parameters or calibration.
    if !state.present || gauge != GaugeState::Ready {
        return Ok(state);
    }
    state.voltage_mv = Some(word(bus, 0x08)?).filter(|value| *value <= 6000);
    state.current_ma = Some(word(bus, 0x0c)? as i16);
    state.average_current_ma = Some(word(bus, 0x14)? as i16);
    state.average_power_mw = Some(word(bus, 0x24)? as i16);
    let temperature = word(bus, 0x28)?;
    state.gauge_temperature_centi_c = (temperature != 0 && temperature != u16::MAX)
        .then_some(i32::from(temperature) * 10 - 27_315);
    state.state_of_charge_percent = percent(word(bus, 0x2c)?);
    state.state_of_health_percent = percent(word(bus, 0x2e)?);
    state.remaining_capacity_mah = Some(word(bus, 0x10)?);
    state.full_capacity_mah = Some(word(bus, 0x12)?);
    state.design_capacity_mah = Some(word(bus, 0x3c)?);
    state.cycles = Some(word(bus, 0x2a)?);
    state.time_to_empty_minutes = minutes(word(bus, 0x16)?);
    state.time_to_full_minutes = minutes(word(bus, 0x18)?);
    // Reject a sample if battery presence or measurement readiness changes during access.
    if (word(bus, 0x0a)? ^ status) & 0x08 != 0 || (word(bus, 0x3a)? ^ operation) & 0x0421 != 0 {
        return Err(Error::ChangedDuringRead);
    }
    Ok(state)
}

fn percent(value: u16) -> Option<u8> {
    (value <= 100).then_some(value as u8)
}

fn minutes(value: u16) -> Option<u16> {
    (value != u16::MAX).then_some(value)
}
