//! Hardware clock with UTC retention and GNSS synchronization.

mod device;
mod service;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::Instant;
use log::error;
use paper_mesh_platform::{Clock, location::Location, time::UtcDateTime};

use crate::{bus::Bus, gnss::FirmwareLocation};
use service::{POLL_MS, Service};

static STATE: Mutex<CriticalSectionRawMutex, RefCell<Service>> =
    Mutex::new(RefCell::new(Service::INITIAL));

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
pub async fn worker(bus: &'static Bus) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Rtc);
    let mut service = service::Service::INITIAL;
    let mut location = FirmwareLocation;
    let mut faulted = false;
    loop {
        suspend::checkpoint(Service::Rtc).await;
        let result = {
            let mut bus = bus.lock().await;
            let now_ms = Instant::now().as_millis();
            service.poll(&mut *bus, location.snapshot(now_ms), now_ms)
        };
        STATE.lock(|state| *state.borrow_mut() = service);
        if let Err(error) = &result
            && !faulted
        {
            error!("RTC: access failed: {:?}", error);
        }
        faulted = result.is_err();
        suspend::delay(Service::Rtc, POLL_MS).await;
    }
}
