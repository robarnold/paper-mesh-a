//! Exercise calendar synchronization without a device driver or executor.

use super::{Policy, Service};
use crate::{
    location::{LocationAvailability, LocationSnapshot, UtcObservation, UtcTime},
    rtc::{InvalidTimeReason, Rtc, RtcError},
    time::{Date, Month, Time, UtcDateTime},
};

const POLICY: Policy = Policy {
    poll_interval_ms: 50,
    source_max_age_ms: 100,
    rtc_max_age_ms: 200,
    correction_threshold_secs: 5,
    first_year: 2100,
    last_year: 2199,
};

struct Calendar {
    utc: Option<UtcDateTime>,
    writes: usize,
    fail: bool,
}

impl Rtc for Calendar {
    type AccessError = ();

    fn read_utc(&mut self) -> Result<UtcDateTime, RtcError<()>> {
        if self.fail {
            return Err(RtcError::Access(()));
        }
        self.utc
            .ok_or(RtcError::InvalidTime(InvalidTimeReason::InvalidCalendar))
    }

    fn set_utc(&mut self, utc: UtcDateTime) -> Result<(), RtcError<()>> {
        self.utc = Some(utc);
        self.writes += 1;
        Ok(())
    }
}

fn utc(second: u8) -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2101, Month::January, 1).unwrap(),
        Time::from_hms(0, 0, second).unwrap(),
    )
}

fn source(second: u8, observed_at_ms: u64) -> LocationSnapshot {
    LocationSnapshot {
        availability: LocationAvailability::Ready,
        utc: Some(UtcObservation {
            observed_at_ms,
            date: Some(utc(second).date()),
            time: UtcTime::new(0, 0, second).unwrap(),
        }),
        ..LocationSnapshot::new()
    }
}

// Alternate years, age limits, and drift thresholds must come from caller policy.
#[test]
fn policy_controls_synchronization_without_a_specific_rtc() {
    let mut calendar = Calendar {
        utc: None,
        writes: 0,
        fail: false,
    };
    let mut service = Service::new(POLICY);
    service.poll(&mut calendar, source(0, 0), 0).unwrap();
    assert_eq!(calendar.writes, 1);
    assert_eq!(service.now_utc(199), Some(utc(0)));
    assert_eq!(service.now_utc(200), None);
    service.poll(&mut calendar, source(4, 10), 10).unwrap();
    assert_eq!(calendar.writes, 1);
    service.poll(&mut calendar, source(5, 20), 20).unwrap();
    assert_eq!(calendar.writes, 2);
    assert_eq!(service.now_utc(20), Some(utc(5)));
    service.poll(&mut calendar, source(20, 30), 130).unwrap();
    assert_eq!(calendar.writes, 2);
}

// Late polls and failed access must schedule one future poll without a retry burst.
#[test]
fn deadlines_advance_after_success_and_failure() {
    let mut calendar = Calendar {
        utc: Some(utc(0)),
        writes: 0,
        fail: false,
    };
    let mut service = Service::new(POLICY);
    assert_eq!(service.next_deadline_ms(), 0);
    service
        .poll(&mut calendar, LocationSnapshot::new(), 10)
        .unwrap();
    assert_eq!(service.next_deadline_ms(), 60);
    calendar.fail = true;
    assert_eq!(
        service.poll(&mut calendar, source(5, 500), 500),
        Err(RtcError::Access(()))
    );
    assert_eq!(service.next_deadline_ms(), 550);
    assert_eq!(service.now_utc(500), None);
    calendar.fail = false;
    service.poll(&mut calendar, source(5, 500), 550).unwrap();
    assert_eq!(service.now_utc(550), Some(utc(5)));
    assert_eq!(service.next_deadline_ms(), 600);
}
