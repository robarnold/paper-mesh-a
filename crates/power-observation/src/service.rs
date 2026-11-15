//! Observation storage, publication, and worker preparation.
//!
//! # Design choices
//!
//! - Keep cache publication and notification together so readers always observe published state.
//! - Own pause storage while returning separate observation and lifecycle capabilities.
//!
//! See [power observation ownership](../../../docs/adapters/power-observation.md).

use crate::{Lifecycle, Observer, Workers};
use core::{cell::RefCell, fmt::Debug, future::Future};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use paper_mesh_platform::{
    power::{BatteryGauge, ChargerObserver, Power, PowerSnapshot},
    worker::Pause,
};
use static_cell::StaticCell;

/// Cached observations, change notifications, and pause storage for one observation worker.
pub struct Service {
    state: Mutex<CriticalSectionRawMutex, RefCell<PowerSnapshot>>,
    changed: Signal<CriticalSectionRawMutex, ()>,
    pause: StaticCell<Pause<CriticalSectionRawMutex>>,
}

impl Default for Service {
    fn default() -> Self {
        Self::new()
    }
}

impl Service {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(PowerSnapshot::INITIAL)),
            changed: Signal::new(),
            pause: StaticCell::new(),
        }
    }

    /// Wait for a publication. Notifications coalesce and support one waiting consumer.
    /// Canceling this wait does not discard a pending notification.
    pub async fn changed(&self) {
        self.changed.wait().await;
    }

    fn publish(&self, snapshot: PowerSnapshot) {
        self.state.lock(|state| *state.borrow_mut() = snapshot);
        self.changed.signal(());
    }

    /// Sample and publish directly while the periodic worker is quiescent.
    /// The caller must coordinate exclusive device use and await completion before releasing the worker.
    pub async fn read_now<G: BatteryGauge, C: ChargerObserver>(
        &self,
        observer: &mut Observer<G, C>,
        now: impl Fn() -> u64,
    ) -> PowerSnapshot {
        let snapshot = observer.read(now).await;
        self.publish(snapshot);
        snapshot
    }

    /// Prepare once after device resources are ready. Launch through the returned worker set.
    /// Panics on repeated preparation. The delay must be nonzero and tolerate cancellation.
    pub fn prepare<G, C, F>(
        &'static self,
        observer: Observer<G, C>,
        now: impl Fn() -> u64 + 'static,
        delay: impl FnMut() -> F + 'static,
    ) -> (Lifecycle, Workers)
    where
        G: BatteryGauge + 'static,
        C: ChargerObserver + 'static,
        G::Error: Debug,
        C::Error: Debug,
        F: Future<Output = ()> + 'static,
    {
        let (mut pause, worker) = self.pause.init(Pause::new()).split();
        // Preparation receives usable device capabilities before enabling the first acquisition.
        pause.resume();
        (
            Lifecycle { pause },
            observer.prepare(worker, now, delay, move |snapshot| self.publish(snapshot)),
        )
    }
}

impl Power for &Service {
    fn snapshot(&mut self, _now_ms: u64) -> PowerSnapshot {
        self.state.lock(|state| *state.borrow())
    }
}

#[cfg(test)]
mod tests;
