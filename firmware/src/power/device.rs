//! BQ27220 and BQ25896 observations through embedded-hal.
//! Register pointer writes do not change charger settings or gauge configuration.

use embedded_hal::i2c::I2c;
use paper_mesh_platform::power::{
    BatteryState, ChargeFault, ChargeState, ChargerFaults, ChargerState, GaugeState, PowerSource,
    ThermistorState,
};

const GAUGE: u8 = 0x55;
const CHARGER: u8 = 0x6b;

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

fn byte<I: I2c>(bus: &mut I, register: u8) -> Result<u8, Error<I::Error>> {
    let mut bytes = [0];
    bus.write_read(CHARGER, &[register], &mut bytes)
        .map_err(Error::Bus)?;
    Ok(bytes[0])
}

pub fn battery<I: I2c>(bus: &mut I) -> Result<BatteryState, Error<I::Error>> {
    let status = word(bus, 0x0a)?;
    let operation = word(bus, 0x3a)?;
    let gauge = if operation & 0x0400 != 0 {
        GaugeState::Configuring
    } else if operation & 1 != 0 {
        GaugeState::Calibrating
    } else if operation & 0x20 == 0 {
        GaugeState::Initializing
    } else {
        GaugeState::Ready
    };
    let mut state = BatteryState {
        present: status & 8 != 0,
        gauge,
        fully_charged: status & 0x0200 != 0,
        fully_discharged: status & 0x8000 != 0,
        discharge_active: status & 1 != 0,
        charge_inhibited: status & 0x0100 != 0,
        overtemperature: status & 0x0c00 != 0,
        shutdown_requested: status & 2 != 0,
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
    if (word(bus, 0x0a)? ^ status) & 8 != 0 || (word(bus, 0x3a)? ^ operation) & 0x0421 != 0 {
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
