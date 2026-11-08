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
use embassy_futures::select::select;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::{Instant, Timer};
use log::error;
use paper_mesh_platform::{
    Clock,
    location::LocationSnapshot,
    power_off::PowerOffParticipant,
    suspend::SuspendParticipant,
    time::UtcDateTime,
    worker::{Pause, PauseControl, PauseWorker},
};

use crate::bus::Bus;
use service::{Policy, Service};

/// Runtime-owned calendar cache and pause channel.
pub struct ClockState {
    service: Mutex<CriticalSectionRawMutex, RefCell<Service>>,
    pause: Pause<CriticalSectionRawMutex>,
}

impl ClockState {
    pub const fn new(policy: Policy) -> Self {
        Self {
            service: Mutex::new(RefCell::new(Service::new(policy))),
            pause: Pause::new(),
        }
    }

    /// Give the clock and worker matching state and unique pause endpoints.
    pub fn split(&'static mut self) -> (FirmwareClock, ClockWorker) {
        let (control, worker) = self.pause.split();
        (
            FirmwareClock {
                state: &self.service,
                pause: control,
            },
            ClockWorker {
                state: &self.service,
                pause: worker,
            },
        )
    }
}

pub struct FirmwareClock {
    state: &'static Mutex<CriticalSectionRawMutex, RefCell<Service>>,
    pause: PauseControl<'static, CriticalSectionRawMutex>,
}

/// Worker endpoint paired with the clock handle during runtime initialization.
pub struct ClockWorker {
    state: &'static Mutex<CriticalSectionRawMutex, RefCell<Service>>,
    pause: PauseWorker<'static, CriticalSectionRawMutex>,
}

impl Clock for FirmwareClock {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }

    fn now_utc(&self) -> Option<UtcDateTime> {
        self.state
            .lock(|state| state.borrow().now_utc(self.now_ms()))
    }
}

impl FirmwareClock {
    /// Request a worker pause before reset recovery without claiming it has completed.
    pub fn request_pause(&mut self) {
        self.pause.request_pause();
    }

    async fn prepare_pause(&mut self) {
        // Pause acknowledgement follows bus release and cache publication.
        self.pause.pause().await;
        self.invalidate_cache();
    }

    fn invalidate_cache(&mut self) {
        // Restore admission separately from freshness. The worker reads the RTC after release.
        self.state
            .lock(|state| state.borrow_mut().invalidate_cache());
    }
}

impl SuspendParticipant for FirmwareClock {
    type Error = core::convert::Infallible;

    async fn will_suspend(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.prepare_pause().await;
        Ok(())
    }

    async fn did_resume(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        self.invalidate_cache();
        self.pause.resume();
        Ok(())
    }

    async fn did_cancel_suspend(&mut self, deadline_ms: u64) -> Result<(), Self::Error> {
        self.did_resume(deadline_ms).await
    }
}

impl PowerOffParticipant for FirmwareClock {
    type Error = core::convert::Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        // Completed worker writes already reside in the RTC. Leave its retained counter running.
        self.prepare_pause().await;
        Ok(())
    }
}

#[embassy_executor::task]
pub async fn worker(bus: &'static Bus, location: fn(u64) -> LocationSnapshot, worker: ClockWorker) {
    let ClockWorker { state, mut pause } = worker;
    let mut faulted = false;
    loop {
        pause.checkpoint().await;
        // Lifecycle callbacks can invalidate the cache while the worker is paused.
        let mut service = state.lock(|state| *state.borrow());
        let result = {
            let mut bus = bus.lock().await;
            let now_ms = Instant::now().as_millis();
            service.poll(&mut Pcf8563::new(&mut *bus), location(now_ms), now_ms)
        };
        state.lock(|state| *state.borrow_mut() = service);
        if let Err(error) = &result
            && !faulted
        {
            error!("RTC: access failed: {:?}", error);
        }
        faulted = result.is_err();
        select(
            Timer::at(Instant::from_millis(service.next_deadline_ms())),
            pause.requested(),
        )
        .await;
    }
}
