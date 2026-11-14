//! Verify charger decoding and read failures independently of firmware.
//!
//! # Design choices
//!
//! - Model fault-latch consumption separately from current fault state.
//!
//! See [Charger observations](../../../docs/adapters/bq25896.md#observations).

use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};
use paper_mesh_bq25896::observation as device;
use paper_mesh_platform::power::{ChargeFault, ChargeState, PowerSource, ThermistorState};

struct Bus {
    charger: [u8; 21],
    latched: u8,
    faults_read: usize,
    transactions: usize,
    fail_at: Option<usize>,
}

impl Bus {
    fn new() -> Self {
        let mut bus = Self {
            charger: [0; 21],
            latched: 0x10,
            faults_read: 0,
            transactions: 0,
            fail_at: None,
        };
        bus.charger[0x0b] = 0x36;
        bus.charger[0x00] = 0x48;
        bus.charger[0x03] = 0x1a;
        bus.charger[0x04] = 16;
        bus.charger[0x05] = 0x11;
        bus.charger[0x06] = 0x5e;
        bus.charger[0x0d] = 18;
        bus
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
        let [Operation::Write(pointer), Operation::Read(bytes)] = operations else {
            panic!("power reads must not write configuration");
        };
        assert_eq!(pointer.len(), 1, "only a register pointer can be written");
        let register = pointer[0] as usize;
        assert_eq!(address, 0x6b);
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
        Ok(())
    }
}

// Charger decoding preserves source, charging phase, and separate fault latches.
#[test]
fn charger_distinguishes_source_phase_settings_and_fault_latches() {
    let mut bus = Bus::new();
    bus.charger[0x0c] = 0xee;
    bus.charger[0x13] = 0xc0;
    bus.charger[0x0e] = 0x80;
    let c = device::read(&mut bus).unwrap();
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
        let c = device::read(&mut bus).unwrap();
        assert_eq!(c.source, source);
        assert_eq!(c.charge, phase);
    }
}

// A failed transaction rejects the partial observation. A later complete read can recover.
#[test]
fn every_bus_failure_discards_the_partial_sample() {
    let mut probe = Bus::new();
    device::read(&mut probe).unwrap();
    for fail in 1..=probe.transactions {
        let mut bus = Bus::new();
        bus.fail_at = Some(fail);
        assert_eq!(
            device::read(&mut bus),
            Err(device::Error::Bus(ErrorKind::Other))
        );
        bus.fail_at = None;
        assert!(device::read(&mut bus).is_ok());
    }
}

// A held bus delays observations. Completed reads release it after success or failure.
#[test]
fn observation_capability_releases_complete_sample_ownership() {
    use core::future::Future;
    use core::task::{Context, Poll, Waker};
    use paper_mesh_platform::power::ChargerObserver;

    let access = paper_mesh_platform::bus::Arbitrated::<
        embassy_sync::blocking_mutex::raw::NoopRawMutex,
        _,
        2,
    >::new(Bus::new());
    let mut device = device::Observer::new(&access);
    let mut cx = Context::from_waker(Waker::noop());
    {
        let owner = access.try_lock().unwrap();
        let mut read = core::pin::pin!(device.read());
        assert!(read.as_mut().poll(&mut cx).is_pending());
        drop(owner);
        assert!(matches!(read.as_mut().poll(&mut cx), Poll::Ready(Ok(_))));
    }
    assert!(matches!(
        core::pin::pin!(device.read()).as_mut().poll(&mut cx),
        Poll::Ready(Ok(_))
    ));
    {
        let mut bus = access.try_lock().unwrap();
        bus.fail_at = Some(bus.transactions + 1);
    }
    assert!(matches!(
        core::pin::pin!(device.read()).as_mut().poll(&mut cx),
        Poll::Ready(Err(device::Error::Bus(ErrorKind::Other)))
    ));
    assert!(access.try_lock().is_some());
}
