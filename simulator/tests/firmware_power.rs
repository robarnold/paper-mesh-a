//! Native tests execute the firmware register adapter without hardware.

#[path = "../../firmware/src/power/device.rs"]
mod device;

use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};
use paper_mesh_platform::power::{
    ChargeFault, ChargeState, GaugeState, PowerSource, ThermistorState,
};

struct Bus {
    gauge: [u8; 64],
    charger: [u8; 21],
    latched: u8,
    faults_read: usize,
    transactions: usize,
    fail_at: Option<usize>,
    remove_at: Option<usize>,
}

impl Bus {
    fn new() -> Self {
        let mut bus = Self {
            gauge: [0; 64],
            charger: [0; 21],
            latched: 0x10,
            faults_read: 0,
            transactions: 0,
            fail_at: None,
            remove_at: None,
        };
        for (register, value) in [
            (0x0a, 9),
            (0x3a, 0x20),
            (0x08, 3890),
            (0x0c, (-180_i16) as u16),
            (0x14, (-175_i16) as u16),
            (0x24, (-700_i16) as u16),
            (0x28, 2989),
            (0x2c, 68),
            (0x2e, 94),
            (0x10, 1918),
            (0x12, 2820),
            (0x3c, 3000),
            (0x2a, 42),
            (0x16, 639),
            (0x18, u16::MAX),
        ] {
            bus.word(register, value);
        }
        bus.charger[0x0b] = 0x36;
        bus.charger[0x00] = 0x48;
        bus.charger[0x03] = 0x1a;
        bus.charger[0x04] = 16;
        bus.charger[0x05] = 0x11;
        bus.charger[0x06] = 0x5e;
        bus.charger[0x0d] = 18;
        bus
    }

    fn word(&mut self, register: usize, value: u16) {
        self.gauge[register..register + 2].copy_from_slice(&value.to_le_bytes());
    }
}

impl ErrorType for Bus {
    type Error = ErrorKind;
}
impl I2c for Bus {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.transactions += 1;
        if self.fail_at == Some(self.transactions) {
            return Err(ErrorKind::Other);
        }
        if self.remove_at == Some(self.transactions) {
            self.word(0x0a, 0);
        }
        let [Operation::Write(pointer), Operation::Read(bytes)] = operations else {
            panic!("power reads must not write configuration");
        };
        assert_eq!(pointer.len(), 1, "only a register pointer can be written");
        let register = pointer[0] as usize;
        match address {
            0x55 => {
                assert_eq!(bytes.len(), 2);
                bytes.copy_from_slice(&self.gauge[register..register + 2]);
            }
            0x6b => {
                assert_eq!(bytes.len(), 1);
                bytes[0] = if register == 0x0c {
                    self.faults_read += 1;
                    if self.faults_read == 1 {
                        self.latched
                    } else {
                        self.charger[register]
                    }
                } else {
                    self.charger[register]
                };
            }
            _ => panic!("unexpected address"),
        }
        Ok(())
    }
}

#[test]
fn gauge_decodes_units_signs_capacity_and_invalid_time() {
    let mut bus = Bus::new();
    let b = device::battery(&mut bus).unwrap();
    assert!(b.present && b.discharge_active);
    assert_eq!(b.gauge, GaugeState::Ready);
    assert!(!b.estimates_verified);
    assert_eq!(b.voltage_mv, Some(3890));
    assert_eq!(b.current_ma, Some(-180));
    assert_eq!(b.average_current_ma, Some(-175));
    assert_eq!(b.average_power_mw, Some(-700));
    assert_eq!(b.gauge_temperature_centi_c, Some(2575));
    assert_eq!(b.state_of_charge_percent, Some(68));
    assert_eq!(b.state_of_health_percent, Some(94));
    assert_eq!(b.remaining_capacity_mah, Some(1918));
    assert_eq!(b.full_capacity_mah, Some(2820));
    assert_eq!(b.design_capacity_mah, Some(3000));
    assert_eq!(b.cycles, Some(42));
    assert_eq!(b.time_to_empty_minutes, Some(639));
    assert_eq!(b.time_to_full_minutes, None);
    bus.word(0x0c, 640);
    bus.word(0x14, 620);
    bus.word(0x28, 2631);
    bus.word(0x16, u16::MAX);
    bus.word(0x18, 0);
    bus.word(0x2c, 0);
    let b = device::battery(&mut bus).unwrap();
    assert_eq!(b.current_ma, Some(640));
    assert_eq!(b.average_current_ma, Some(620));
    assert_eq!(b.gauge_temperature_centi_c, Some(-1005));
    assert_eq!(b.time_to_empty_minutes, None);
    assert_eq!(b.time_to_full_minutes, Some(0));
    assert_eq!(b.state_of_charge_percent, Some(0));
}

#[test]
fn gauge_absence_readiness_and_invalid_values_do_not_invent_measurements() {
    for (status, operation, expected) in [
        (0, 0x20, GaugeState::Ready),
        (8, 0, GaugeState::Initializing),
        (8, 0x420, GaugeState::Configuring),
        (8, 0x21, GaugeState::Calibrating),
    ] {
        let mut bus = Bus::new();
        bus.word(0x0a, status);
        bus.word(0x3a, operation);
        let b = device::battery(&mut bus).unwrap();
        assert_eq!(b.present, status != 0);
        assert_eq!(b.gauge, expected);
        assert_eq!(b.voltage_mv, None);
        assert_eq!(b.state_of_charge_percent, None);
        assert_eq!(bus.transactions, 2);
    }
    let mut bus = Bus::new();
    for register in [0x2c, 0x2e, 0x28, 0x08] {
        bus.word(register, u16::MAX);
    }
    let b = device::battery(&mut bus).unwrap();
    assert_eq!(b.voltage_mv, None);
    assert_eq!(b.state_of_charge_percent, None);
    assert_eq!(b.state_of_health_percent, None);
    assert_eq!(b.gauge_temperature_centi_c, None);
}

#[test]
fn battery_flags_and_mid_sample_removal_are_distinct() {
    let mut bus = Bus::new();
    bus.word(0x0a, 0x8f0b);
    let b = device::battery(&mut bus).unwrap();
    assert!(
        b.fully_charged
            && b.fully_discharged
            && b.charge_inhibited
            && b.overtemperature
            && b.shutdown_requested
    );
    let mut bus = Bus::new();
    bus.remove_at = Some(4);
    assert_eq!(
        device::battery(&mut bus),
        Err(device::Error::ChangedDuringRead)
    );
}

#[test]
fn charger_distinguishes_source_phase_settings_and_fault_latches() {
    let mut bus = Bus::new();
    bus.charger[0x0c] = 0xee;
    bus.charger[0x13] = 0xc0;
    bus.charger[0x0e] = 0x80;
    let c = device::charger(&mut bus).unwrap();
    assert_eq!(c.source, PowerSource::Usb);
    assert_eq!(c.charge, ChargeState::Fast);
    assert!(c.power_good && c.charge_enabled && !c.input_suspended);
    assert_eq!(c.input_limit_ma, 500);
    assert_eq!(c.charge_limit_ma, 1024);
    assert_eq!(c.charge_voltage_mv, 4208);
    assert_eq!(c.termination_current_ma, 128);
    assert_eq!(c.input_voltage_limit_mv, 4400);
    assert!(c.input_current_limited && c.input_voltage_limited && c.thermal_regulation);
    assert_eq!(c.latched_faults.charge, ChargeFault::Input);
    assert_eq!(c.faults.charge, ChargeFault::ThermalShutdown);
    assert_eq!(c.faults.thermistor, ThermistorState::Hot);
    assert!(c.faults.battery_overvoltage && c.faults.watchdog_expired && c.faults.boost);
    assert_eq!(bus.faults_read, 2);
    for (raw, source, phase) in [
        (0x02, PowerSource::None, ChargeState::Idle),
        (0x4e, PowerSource::Adapter, ChargeState::Precharge),
        (0x3e, PowerSource::Usb, ChargeState::Complete),
        (0xe2, PowerSource::Output, ChargeState::Idle),
        (0x62, PowerSource::Unknown, ChargeState::Idle),
    ] {
        bus.charger[0x0b] = raw;
        let c = device::charger(&mut bus).unwrap();
        assert_eq!(c.source, source);
        assert_eq!(c.charge, phase);
    }
}

#[test]
fn every_bus_failure_discards_the_partial_sample_and_next_read_recovers() {
    let mut probe = Bus::new();
    device::battery(&mut probe).unwrap();
    let battery_reads = probe.transactions;
    for fail in 1..=battery_reads {
        let mut bus = Bus::new();
        bus.fail_at = Some(fail);
        assert!(device::battery(&mut bus).is_err());
        assert!(device::charger(&mut bus).is_ok());
        bus.fail_at = None;
        assert!(device::battery(&mut bus).is_ok());
    }
    let mut probe = Bus::new();
    device::charger(&mut probe).unwrap();
    for fail in 1..=probe.transactions {
        let mut bus = Bus::new();
        bus.fail_at = Some(fail);
        assert!(device::charger(&mut bus).is_err());
        assert!(device::battery(&mut bus).is_ok());
        bus.fail_at = None;
        assert!(device::charger(&mut bus).is_ok());
    }
}
