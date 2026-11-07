//! Hardware clock with UTC retention and GNSS synchronization.
//!
//! # Design choices
//!
//! - Connect the shared bus clock to GPS synchronization so valid retained time
//!   remains available between receiver reports.
//!
//! See [T5 S3 Pro real-time clock](../../../docs/boards/lilygo-t5-s3-pro-rtc.md).

use paper_mesh_pcf8563::Pcf8563;
use paper_mesh_platform::rtc::synchronization as service;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::Instant;
use log::error;
use paper_mesh_platform::{Clock, location::LocationSnapshot, time::UtcDateTime};

use crate::bus::Bus;
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

static STATE: Mutex<CriticalSectionRawMutex, RefCell<Service>> =
    Mutex::new(RefCell::new(Service::new(POLICY)));

pub struct FirmwareClock;

impl Clock for FirmwareClock {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }

    fn now_utc(&self) -> Option<UtcDateTime> {
        STATE.lock(|state| state.borrow().now_utc(self.now_ms()))
    }
}

#[embassy_executor::task]
pub async fn worker(bus: &'static Bus, location: fn(u64) -> LocationSnapshot) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Rtc);
    let mut service = service::Service::new(POLICY);
    let mut faulted = false;
    loop {
        suspend::checkpoint(Service::Rtc).await;
        let result = {
            let mut bus = bus.lock().await;
            let now_ms = Instant::now().as_millis();
            service.poll(&mut Pcf8563::new(&mut *bus), location(now_ms), now_ms)
        };
        STATE.lock(|state| *state.borrow_mut() = service);
        if let Err(error) = &result
            && !faulted
        {
            error!("RTC: access failed: {:?}", error);
        }
        faulted = result.is_err();
        suspend::delay(
            Service::Rtc,
            service
                .next_deadline_ms()
                .saturating_sub(Instant::now().as_millis()),
        )
        .await;
    }
}
