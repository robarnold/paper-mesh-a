//! BQ25896 charger observations through embedded-hal.
//!
//! # Design choices
//!
//! - Read latched and current faults separately so past faults do not appear current.
//! - Register pointer writes do not change charger settings.
//!
//! See [Charger observations](../../../docs/boards/lilygo-t5-s3-pro-power.md#charger).

use embedded_hal::i2c::I2c;
use paper_mesh_platform::power::{
    ChargeFault, ChargeState, ChargerFaults, ChargerState, PowerSource, ThermistorState,
};

const CHARGER: u8 = 0x6b;

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
}

fn byte<I: I2c>(bus: &mut I, register: u8) -> Result<u8, Error<I::Error>> {
    let mut bytes = [0];
    bus.write_read(CHARGER, &[register], &mut bytes)
        .map_err(Error::Bus)?;
    Ok(bytes[0])
}

pub fn charger<I: I2c>(bus: &mut I) -> Result<ChargerState, Error<I::Error>> {
    let status = byte(bus, 0x0b)?;
    // REG0C does not support a burst read. Two reads distinguish latched and current faults.
    let latched_faults = faults(byte(bus, 0x0c)?);
    let current_faults = faults(byte(bus, 0x0c)?);
    let input = byte(bus, 0x00)?;
    let control = byte(bus, 0x03)?;
    let charge_limit_ma = u16::from((byte(bus, 0x04)? & 0x7f).min(47)) * 64;
    let termination_current_ma = (u16::from(byte(bus, 0x05)? & 0x0f) + 1) * 64;
    let charge_voltage_mv = 3840 + u16::from((byte(bus, 0x06)? >> 2).min(48)) * 16;
    let input_voltage_limit_mv = 2600 + u16::from((byte(bus, 0x0d)? & 0x7f).max(13)) * 100;
    let thermal_regulation = byte(bus, 0x0e)? & 0x80 != 0;
    let regulation = byte(bus, 0x13)?;
    Ok(ChargerState {
        source: match status >> 5 {
            0 => PowerSource::None,
            1 => PowerSource::Usb,
            2 => PowerSource::Adapter,
            7 => PowerSource::Output,
            _ => PowerSource::Unknown,
        },
        charge: match (status >> 3) & 3 {
            0 => ChargeState::Idle,
            1 => ChargeState::Precharge,
            2 => ChargeState::Fast,
            _ => ChargeState::Complete,
        },
        power_good: status & 4 != 0,
        minimum_system_voltage: status & 1 != 0,
        charge_enabled: control & 0x10 != 0,
        input_suspended: input & 0x80 != 0,
        input_limit_ma: 100 + u16::from(input & 0x3f) * 50,
        charge_limit_ma,
        charge_voltage_mv,
        termination_current_ma,
        input_voltage_limit_mv,
        input_current_limited: regulation & 0x40 != 0,
        input_voltage_limited: regulation & 0x80 != 0,
        thermal_regulation,
        faults: current_faults,
        latched_faults,
    })
}

fn faults(value: u8) -> ChargerFaults {
    ChargerFaults {
        watchdog_expired: value & 0x80 != 0,
        boost: value & 0x40 != 0,
        battery_overvoltage: value & 8 != 0,
        charge: match (value >> 4) & 3 {
            0 => ChargeFault::None,
            1 => ChargeFault::Input,
            2 => ChargeFault::ThermalShutdown,
            _ => ChargeFault::SafetyTimer,
        },
        thermistor: match value & 7 {
            0 => ThermistorState::Normal,
            2 => ThermistorState::Warm,
            3 => ThermistorState::Cool,
            5 => ThermistorState::Cold,
            6 => ThermistorState::Hot,
            _ => ThermistorState::Unknown,
        },
    }
}
