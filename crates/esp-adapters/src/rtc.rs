//! Hardware clock with UTC retention and GNSS synchronization.
//!
//! # Design choices
//!
//! - Connect the shared bus clock to GPS synchronization so valid retained time
//!   remains available between receiver reports.
//! - Receive policy, storage, and bus capacity from the runtime so board choices stay outside the adapter.
//! - Box the worker future once so the adapter can own task storage without fixing the shared bus capacity.
//!
//! See [ESP clock integration](../../../docs/adapters/rtc.md#esp-clock-integration).

use paper_mesh_pcf8563::Pcf8563;
use paper_mesh_platform::rtc::synchronization as service;

use alloc::boxed::Box;
use core::{cell::RefCell, future::Future, pin::Pin};
use embassy_executor::{SpawnError, Spawner};
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
    worker::{Pause, PauseControl, PauseWorker, WorkerSet},
};

use esp_hal::{Blocking, i2c::master::I2c};
use paper_mesh_platform::bus::Arbitrated;
use service::{Policy, Service};

/// Runtime-owned calendar cache and pause channel. Initialize once, then split before spawning the worker.
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
    pub fn split(&'static mut self) -> (RtcClock, ClockWorker) {
        let (mut control, worker) = self.pause.split();
        // Calendar state is initialized. Worker preparation supplies the bus before launch.
        control.resume();
        (
            RtcClock {
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

/// Calendar access and lifecycle control paired with the adapter's acquisition worker.
pub struct RtcClock {
    state: &'static Mutex<CriticalSectionRawMutex, RefCell<Service>>,
    pause: PauseControl<'static, CriticalSectionRawMutex>,
}

/// Worker endpoint paired with the clock handle during runtime initialization.
pub struct ClockWorker {
    state: &'static Mutex<CriticalSectionRawMutex, RefCell<Service>>,
    pause: PauseWorker<'static, CriticalSectionRawMutex>,
}

/// Prepared RTC work with bus capacity hidden from the executor task type.
#[must_use]
pub struct Workers {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

impl WorkerSet for Workers {
    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(worker(self)?);
        Ok(())
    }
}

#[embassy_executor::task]
async fn worker(workers: Workers) {
    workers.future.await;
}

impl Clock for RtcClock {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }

    fn now_utc(&self) -> Option<UtcDateTime> {
        self.state
            .lock(|state| state.borrow().now_utc(self.now_ms()))
    }
}

impl RtcClock {
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

impl SuspendParticipant for RtcClock {
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

impl PowerOffParticipant for RtcClock {
    type Error = core::convert::Infallible;

    async fn will_power_off(&mut self, _deadline_ms: u64) -> Result<(), Self::Error> {
        // Completed worker writes already reside in the RTC. Leave its retained counter running.
        self.prepare_pause().await;
        Ok(())
    }
}

impl ClockWorker {
    /// Prepare the worker with the runtime's bus and location source, after allocator initialization.
    /// Allocate its future once to preserve runtime-selected bus capacity without a firmware task wrapper.
    pub fn prepare<const N: usize>(
        self,
        bus: &'static Arbitrated<CriticalSectionRawMutex, I2c<'static, Blocking>, N>,
        location: fn(u64) -> LocationSnapshot,
    ) -> Workers {
        Workers {
            future: Box::pin(self.run(bus, location)),
        }
    }

    async fn run<const N: usize>(
        self,
        bus: &'static Arbitrated<CriticalSectionRawMutex, I2c<'static, Blocking>, N>,
        location: fn(u64) -> LocationSnapshot,
    ) {
        let ClockWorker { state, mut pause } = self;
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
}
