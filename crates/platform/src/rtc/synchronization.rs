//! Calendar synchronization from supplied location observations.
//!
//! # Design choices
//!
//! - Consume a fresh calendar correction once so polling the same report again cannot set
//!   the clock backward.
//!
//! See [Synchronization](../../../../docs/services/time.md#calendar-synchronization).

use crate::{
    location::{LocationAvailability, LocationSnapshot},
    time::{ObservationWindow, UtcDateTime, UtcReference},
};

use super::{Rtc, RtcError};

/// Caller-selected age limits, correction threshold, and supported calendar range.
#[derive(Clone, Copy)]
pub struct Policy {
    pub poll_interval_ms: u64,
    pub source_max_age_ms: u64,
    pub rtc_max_age_ms: u64,
    pub correction_threshold_secs: u64,
    pub first_year: i32,
    pub last_year: i32,
}

#[derive(Clone, Copy)]
pub struct Service {
    policy: Policy,
    next_poll_ms: u64,
    reference: Option<UtcReference>,
    last_gnss_at_ms: Option<u64>,
    synchronized: bool,
}

impl Service {
    pub const fn new(policy: Policy) -> Self {
        assert!(policy.poll_interval_ms > 0);
        Self {
            policy,
            next_poll_ms: 0,
            reference: None,
            last_gnss_at_ms: None,
            synchronized: false,
        }
    }

    /// Return the next polling deadline in the caller's monotonic epoch.
    /// A new service is due immediately. Late polls do not create catch-up work.
    pub fn next_deadline_ms(&self) -> u64 {
        self.next_poll_ms
    }

    /// Require a new RTC read after an interruption without replaying consumed corrections.
    /// The next poll is due immediately in the existing monotonic epoch.
    pub fn invalidate_cache(&mut self) {
        self.reference = None;
        self.next_poll_ms = 0;
    }

    pub fn now_utc(&self, now_ms: u64) -> Option<UtcDateTime> {
        let reference = self.reference?;
        ObservationWindow::new(now_ms, self.policy.rtc_max_age_ms)
            .contains(reference.observed_at_ms)
            .then(|| reference.at(now_ms))
            .flatten()
            .filter(|utc| (self.policy.first_year..=self.policy.last_year).contains(&utc.year()))
    }

    pub fn poll<R: Rtc>(
        &mut self,
        rtc: &mut R,
        location: LocationSnapshot,
        now_ms: u64,
    ) -> Result<(), RtcError<R::AccessError>> {
        self.next_poll_ms = now_ms.saturating_add(self.policy.poll_interval_ms);
        // Do not expose the previous value after a failed calendar operation.
        self.reference = None;
        let mut datetime = read_valid(rtc)?;
        let observation = location
            .utc
            .filter(|_| location.availability == LocationAvailability::Ready)
            .filter(|utc| {
                ObservationWindow::new(now_ms, self.policy.source_max_age_ms)
                    .contains(utc.observed_at_ms)
                    && self
                        .last_gnss_at_ms
                        .is_none_or(|last| utc.observed_at_ms > last)
            })
            .and_then(|utc| utc.reference())
            .filter(|utc| {
                (self.policy.first_year..=self.policy.last_year).contains(&utc.datetime.year())
            });
        if let Some(observation) = observation
            && let Some(gps) = observation.at(now_ms)
        {
            // The first correction sets the RTC. Later reports use the caller's drift threshold.
            let correction = !self.synchronized
                || datetime.is_none_or(|rtc| {
                    rtc.unix_timestamp().abs_diff(gps.unix_timestamp())
                        >= self.policy.correction_threshold_secs
                });
            if correction {
                rtc.set_utc(gps)?;
                datetime = read_valid(rtc)?;
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

fn read_valid<R: Rtc>(rtc: &mut R) -> Result<Option<UtcDateTime>, RtcError<R::AccessError>> {
    match rtc.read_utc() {
        Ok(utc) => Ok(Some(utc)),
        Err(RtcError::InvalidTime(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
