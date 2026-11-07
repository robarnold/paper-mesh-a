//! Native tests execute the firmware RTC driver and synchronization service.
//!
//! # Design choices
//!
//! - Check calendar validity and correction failures without mistaking a successful
//!   register write for verified retention.
//!
//! See [Retained calendar adapter](../../../docs/adapters/rtc.md).

use std::cell::RefCell;

use paper_mesh_pcf8563 as device;
use paper_mesh_platform::rtc::synchronization as service;

use device::Error;
use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};
use paper_mesh_platform::{
    location::{LocationAvailability, LocationSnapshot, UtcObservation, UtcTime},
    time::{Date, Month, Time, UtcDateTime},
};
use service::{Policy, Service};
const POLL_MS: u64 = 1_000;
const POLICY: Policy = Policy {
    poll_interval_ms: POLL_MS,
    source_max_age_ms: 2_000,
    rtc_max_age_ms: 10_000,
    correction_threshold_secs: 2,
    first_year: 2000,
    last_year: 2099,
};

struct Rtc {
    registers: [u8; 16],
    transactions: usize,
    writes: Vec<Vec<u8>>,
    fail_at: Option<usize>,
}

impl Rtc {
    fn retained() -> Self {
        Self {
            // 2026-10-18 23:59:58 UTC, Sunday. The alarm and timer values are unrelated.
            registers: [
                0, 0x13, 0x58, 0x59, 0x23, 0x18, 0, 0x10, 0x26, 1, 2, 3, 4, 5, 6, 7,
            ],
            transactions: 0,
            writes: Vec::new(),
            fail_at: None,
        }
    }
}

impl ErrorType for Rtc {
    type Error = ErrorKind;
}

impl I2c for Rtc {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        assert_eq!(address, 0x51);
        self.transactions += 1;
        let fail = self.fail_at == Some(self.transactions);
        match operations {
            [Operation::Write(pointer), Operation::Read(bytes)] => {
                // A complete calendar read also checks control and validity flags.
                assert_eq!(*pointer, &[0]);
                assert_eq!(bytes.len(), 9);
                if !fail {
                    bytes.copy_from_slice(&self.registers[..9]);
                }
            }
            [Operation::Write(bytes)] => {
                self.writes.push(bytes.to_vec());
                let start = bytes[0] as usize;
                // Model an error after part of a write has reached the device.
                let count = if fail { 1 } else { bytes.len() - 1 };
                self.registers[start..start + count].copy_from_slice(&bytes[1..1 + count]);
            }
            _ => panic!("unexpected transaction"),
        }
        if fail { Err(ErrorKind::Other) } else { Ok(()) }
    }
}

// A separate handle models hardware changes while the driver remains alive.
struct SharedBus<'a>(&'a RefCell<Rtc>);

impl ErrorType for SharedBus<'_> {
    type Error = ErrorKind;
}

impl I2c for SharedBus<'_> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.0.borrow_mut().transaction(address, operations)
    }
}

fn datetime(year: i32, month: Month, day: u8, hour: u8, minute: u8, second: u8) -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(year, month, day).unwrap(),
        Time::from_hms(hour, minute, second).unwrap(),
    )
}

fn gps(datetime: UtcDateTime, observed_at_ms: u64) -> LocationSnapshot {
    LocationSnapshot {
        availability: LocationAvailability::Ready,
        utc: Some(UtcObservation {
            observed_at_ms,
            time: UtcTime::new(datetime.hour(), datetime.minute(), datetime.second()).unwrap(),
            date: Some(datetime.date()),
        }),
        ..LocationSnapshot::new()
    }
}

#[test]
fn retained_calendar_requires_no_write_or_gps_fix() {
    let rtc = RefCell::new(Rtc::retained());
    let mut driver = device::Pcf8563::new(SharedBus(&rtc));
    let mut service = Service::new(POLICY);
    assert_eq!(service.now_utc(0), None);
    service
        .poll(&mut driver, LocationSnapshot::new(), 500)
        .unwrap();
    let expected = datetime(2026, Month::October, 18, 23, 59, 58);
    assert_eq!(service.now_utc(500), Some(expected));
    assert_eq!(
        service.now_utc(2_500),
        Some(datetime(2026, Month::October, 19, 0, 0, 0))
    );
    assert_eq!(service.now_utc(499), None);
    assert_eq!(service.now_utc(10_500), None);
    assert!(rtc.borrow().writes.is_empty());
    // A new service represents a reboot with the same RTC registers.
    let mut reboot = Service::new(POLICY);
    reboot
        .poll(&mut driver, LocationSnapshot::new(), 0)
        .unwrap();
    assert_eq!(reboot.now_utc(0), Some(expected));
}

#[test]
fn invalid_flags_bcd_and_calendar_values_cannot_supply_utc() {
    for (register, value) in [
        (0, 0x20),
        (0, 0x80),
        (0, 0x08),
        (2, 0xd8),
        (2, 0x5a),
        (2, 0x60),
        (3, 0x60),
        (4, 0x24),
        (5, 0),
        (5, 0x32),
        (6, 7),
        (7, 0x90),
        (7, 0),
        (7, 0x13),
        (8, 0xfa),
    ] {
        let mut rtc = Rtc::retained();
        rtc.registers[register] = value;
        assert_eq!(
            device::read(&mut rtc).unwrap(),
            None,
            "register {register}: {value:x}"
        );
        assert!(rtc.writes.is_empty());
    }
    let mut rtc = Rtc::retained();
    rtc.registers[5] = 0x29;
    rtc.registers[7] = 0x02;
    rtc.registers[8] = 0x26;
    assert_eq!(device::read(&mut rtc).unwrap(), None);
    rtc.registers[8] = 0x24;
    assert_eq!(device::read(&mut rtc).unwrap().unwrap().day(), 29);
}

#[test]
fn complete_calendar_write_preserves_other_registers_and_sets_weekday() {
    let mut rtc = Rtc::retained();
    let original = rtc.registers;
    let time = datetime(2024, Month::February, 29, 12, 34, 56);
    device::set(&mut rtc, time).unwrap();
    assert_eq!(
        rtc.writes,
        vec![
            vec![0, 0x20],
            vec![2, 0x56, 0x34, 0x12, 0x29, 4, 0x02, 0x24],
            vec![0, 0]
        ]
    );
    assert_eq!(rtc.registers[1], original[1]);
    assert_eq!(rtc.registers[9..], original[9..]);
    assert_eq!(device::read(&mut rtc).unwrap(), Some(time));
    for year in [1999, 2100] {
        assert_eq!(
            device::set(&mut rtc, datetime(year, Month::January, 1, 0, 0, 0)),
            Err(Error::UnsupportedYear)
        );
    }
    assert_eq!(rtc.writes.len(), 3);
}

#[test]
fn voltage_loss_waits_for_a_complete_fresh_report_then_retains_time_without_gps() {
    let rtc = RefCell::new(Rtc::retained());
    let mut driver = device::Pcf8563::new(SharedBus(&rtc));
    rtc.borrow_mut().registers[2] |= 0x80;
    let mut service = Service::new(POLICY);
    service
        .poll(&mut driver, LocationSnapshot::new(), 0)
        .unwrap();
    assert_eq!(service.now_utc(0), None);
    let report = gps(datetime(2024, Month::December, 31, 23, 59, 59), 100);
    service.poll(&mut driver, report, 1_100).unwrap();
    assert_eq!(
        service.now_utc(1_100),
        Some(datetime(2025, Month::January, 1, 0, 0, 0))
    );
    // Independent RTC ticks continue after receiver loss.
    rtc.borrow_mut().registers[2] = 1;
    service
        .poll(&mut driver, LocationSnapshot::new(), 1_100 + POLL_MS)
        .unwrap();
    assert_eq!(
        service.now_utc(2_100),
        Some(datetime(2025, Month::January, 1, 0, 0, 1))
    );
    assert_eq!(rtc.borrow().writes.len(), 3);
}

#[test]
fn stale_future_incomplete_and_faulted_gps_reports_cannot_set_the_rtc() {
    let report = gps(datetime(2026, Month::January, 1, 0, 0, 0), 100);
    let mut no_date = report;
    no_date.utc.as_mut().unwrap().date = None;
    let mut fault = report;
    fault.availability = LocationAvailability::Fault;
    for (report, now_ms) in [(report, 99), (report, 2_100), (no_date, 100), (fault, 100)] {
        let rtc = RefCell::new(Rtc::retained());
        let mut driver = device::Pcf8563::new(SharedBus(&rtc));
        rtc.borrow_mut().registers[2] |= 0x80;
        let mut service = Service::new(POLICY);
        service.poll(&mut driver, report, now_ms).unwrap();
        assert_eq!(service.now_utc(now_ms), None);
        assert!(rtc.borrow().writes.is_empty());
    }
}

#[test]
fn repeated_reports_do_not_reset_the_counter_and_new_reports_correct_drift() {
    let rtc = RefCell::new(Rtc::retained());
    let mut driver = device::Pcf8563::new(SharedBus(&rtc));
    let mut service = Service::new(POLICY);
    let report = gps(datetime(2026, Month::October, 18, 23, 59, 58), 0);
    service.poll(&mut driver, report, 0).unwrap();
    service.poll(&mut driver, report, POLL_MS).unwrap();
    assert_eq!(rtc.borrow().writes.len(), 3);
    let close = gps(datetime(2026, Month::October, 18, 23, 59, 59), POLL_MS);
    service.poll(&mut driver, close, POLL_MS).unwrap();
    assert_eq!(rtc.borrow().writes.len(), 3);
    for (now_ms, correction) in [
        (2 * POLL_MS, datetime(2026, Month::October, 19, 0, 0, 2)),
        (3 * POLL_MS, datetime(2026, Month::October, 18, 23, 59, 55)),
    ] {
        service
            .poll(&mut driver, gps(correction, now_ms), now_ms)
            .unwrap();
        assert_eq!(service.now_utc(now_ms), Some(correction));
    }
    assert_eq!(rtc.borrow().writes.len(), 9);
    service.poll(&mut driver, report, 3 * POLL_MS).unwrap();
    assert_eq!(rtc.borrow().writes.len(), 9);
}

#[test]
fn each_bus_failure_invalidates_the_clock_and_allows_a_complete_retry() {
    // Poll has a read, STOP, calendar write, start, and verification read.
    for fail_at in 1..=5 {
        let rtc = RefCell::new(Rtc::retained());
        let mut driver = device::Pcf8563::new(SharedBus(&rtc));
        let mut service = Service::new(POLICY);
        service
            .poll(&mut driver, LocationSnapshot::new(), 0)
            .unwrap();
        rtc.borrow_mut().transactions = 0;
        rtc.borrow_mut().fail_at = Some(fail_at);
        let correction = datetime(2024, Month::February, 29, 12, 34, 56);
        let report = gps(correction, 100);
        assert!(service.poll(&mut driver, report, 100).is_err());
        assert_eq!(service.now_utc(100), None);
        rtc.borrow_mut().fail_at = None;
        if fail_at == 2 || fail_at == 3 {
            assert_eq!(device::read(&mut *rtc.borrow_mut()).unwrap(), None);
        }
        service.poll(&mut driver, report, 101).unwrap();
        assert_eq!(service.now_utc(101), Some(correction));
    }
}

#[test]
fn rtc_loss_after_synchronization_requires_a_new_report() {
    let rtc = RefCell::new(Rtc::retained());
    let mut driver = device::Pcf8563::new(SharedBus(&rtc));
    let mut service = Service::new(POLICY);
    let report = gps(datetime(2026, Month::October, 18, 23, 59, 58), 0);
    service.poll(&mut driver, report, 0).unwrap();
    rtc.borrow_mut().registers[2] |= 0x80;
    service.poll(&mut driver, report, 500).unwrap();
    assert_eq!(service.now_utc(500), None);
    service
        .poll(
            &mut driver,
            gps(report.utc.unwrap().reference().unwrap().datetime, 1_000),
            1_000,
        )
        .unwrap();
    assert!(service.now_utc(1_000).is_some());
}
