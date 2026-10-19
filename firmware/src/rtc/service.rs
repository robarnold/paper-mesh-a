//! RTC observations and corrections from complete GNSS reports.

use embedded_hal::i2c::I2c;
use paper_mesh_platform::{
    location::{LocationAvailability, LocationSnapshot},
    time::{ObservationWindow, UtcDateTime, UtcReference},
};

use super::device::{self, Error};

pub const POLL_MS: u64 = 1_000;
const GNSS_MAX_AGE_MS: u64 = 2_000;
const RTC_MAX_AGE_MS: u64 = 10_000;

#[derive(Clone, Copy)]
pub struct Service {
    reference: Option<UtcReference>,
    last_gnss_at_ms: Option<u64>,
    synchronized: bool,
}

impl Service {
    pub const INITIAL: Self = Self {
        reference: None,
        last_gnss_at_ms: None,
        synchronized: false,
    };

    pub fn now_utc(&self, now_ms: u64) -> Option<UtcDateTime> {
        let reference = self.reference?;
        ObservationWindow::new(now_ms, RTC_MAX_AGE_MS)
            .contains(reference.observed_at_ms)
            .then(|| reference.at(now_ms))
            .flatten()
            .filter(|utc| (2000..=2099).contains(&utc.year()))
    }

    pub fn poll<I: I2c>(
        &mut self,
        bus: &mut I,
        location: LocationSnapshot,
        now_ms: u64,
    ) -> Result<(), Error<I::Error>> {
        // Do not expose the previous value after a failed bus transaction.
        self.reference = None;
        let mut datetime = device::read(bus)?;
        let observation = location
            .utc
            .filter(|_| location.availability == LocationAvailability::Ready)
            .filter(|utc| {
                ObservationWindow::new(now_ms, GNSS_MAX_AGE_MS).contains(utc.observed_at_ms)
                    && self
                        .last_gnss_at_ms
                        .is_none_or(|last| utc.observed_at_ms > last)
            })
            .and_then(|utc| utc.reference())
            .filter(|utc| (2000..=2099).contains(&utc.datetime.year()));
        if let Some(observation) = observation
            && let Some(gps) = observation.at(now_ms)
        {
            // The first fix sets the RTC. Later reports correct drift of two seconds or more.
            let correction = !self.synchronized
                || datetime
                    .is_none_or(|rtc| rtc.unix_timestamp().abs_diff(gps.unix_timestamp()) >= 2);
            if correction {
                device::set(bus, gps)?;
                datetime = device::read(bus)?;
                self.synchronized = datetime.is_some();
            }
            if datetime.is_some() {
                self.last_gnss_at_ms = Some(observation.observed_at_ms);
            }
        }
        self.reference = datetime.map(|datetime| UtcReference {
            datetime,
            observed_at_ms: now_ms,
        });
        Ok(())
    }
}
