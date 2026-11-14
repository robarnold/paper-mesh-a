//! Periodic power observations on core 0 through the shared I2C bus.
//!
//! # Design choices
//!
//! - Publish gauge and charger observations independently so a bus error for one
//!   device does not invalidate the other.
//!
//! See [Hardware and execution](../../../docs/boards/lilygo-t5-s3-pro-power.md#hardware-and-execution).

mod lifecycle;
use super::device;
pub use lifecycle::PowerObservationLifecycle;

use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer};
use paper_mesh_platform::power::{Power, PowerAvailability, PowerReading, PowerSnapshot};
use paper_mesh_platform::worker::{Pause, PauseWorker};
use static_cell::StaticCell;

use crate::bus::Bus;

static STATE: Mutex<CriticalSectionRawMutex, RefCell<PowerSnapshot>> =
    Mutex::new(RefCell::new(PowerSnapshot::INITIAL));
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static PAUSE: StaticCell<Pause<CriticalSectionRawMutex>> = StaticCell::new();

pub struct FirmwarePowerObservation;

impl Power for FirmwarePowerObservation {
    fn snapshot(&mut self, _now_ms: u64) -> PowerSnapshot {
        STATE.lock(|state| *state.borrow())
    }
}

pub async fn changed() {
    CHANGED.wait().await;
}

fn publish(snapshot: PowerSnapshot) {
    STATE.lock(|state| *state.borrow_mut() = snapshot);
    CHANGED.signal(());
}

pub async fn read_now(bus: &Bus) -> PowerSnapshot {
    let battery = {
        let mut bus = bus.lock().await;
        device::battery(&mut *bus)
            .map(|value| PowerReading::ready(value, Instant::now().as_millis()))
            .unwrap_or(PowerReading::unavailable(PowerAvailability::Fault))
    };
    let charger = {
        let mut bus = bus.lock().await;
        device::charger(&mut *bus)
            .map(|value| PowerReading::ready(value, Instant::now().as_millis()))
            .unwrap_or(PowerReading::unavailable(PowerAvailability::Fault))
    };
    let snapshot = PowerSnapshot { battery, charger };
    publish(snapshot);
    snapshot
}

/// Start periodic observations with the runtime's nonzero polling interval.
/// Pauses interrupt the delay after a completed pair of device reads.
pub fn start_observation(
    spawner: Spawner,
    bus: &'static Bus,
    poll_interval: Duration,
) -> PowerObservationLifecycle {
    assert!(poll_interval.as_ticks() > 0);
    let (mut pause, worker_pause) = PAUSE.init(Pause::new()).split();
    // The shared bus is initialized before observation starts.
    pause.resume();
    spawner.spawn(observation_worker(bus, worker_pause, poll_interval).unwrap());
    PowerObservationLifecycle { pause }
}

#[embassy_executor::task]
async fn observation_worker(
    bus: &'static Bus,
    mut pause: PauseWorker<'static, CriticalSectionRawMutex>,
    poll_interval: Duration,
) {
    let mut failed = (false, false);
    loop {
        pause.checkpoint().await;
        let battery = {
            let mut bus = bus.lock().await;
            let at_ms = Instant::now().as_millis();
            device::battery(&mut *bus).map(|value| PowerReading::ready(value, at_ms))
        };
        // Release the bus between devices so other clients can proceed.
        let charger = {
            let mut bus = bus.lock().await;
            let at_ms = Instant::now().as_millis();
            device::charger(&mut *bus).map(|value| PowerReading::ready(value, at_ms))
        };
        if let Err(error) = &battery
            && !failed.0
        {
            log::error!("battery: read failed: {:?}", error);
        }
        if let Err(error) = &charger
            && !failed.1
        {
            log::error!("charger: read failed: {:?}", error);
        }
        failed = (battery.is_err(), charger.is_err());
        publish(PowerSnapshot {
            battery: battery.unwrap_or(PowerReading::unavailable(PowerAvailability::Fault)),
            charger: charger.unwrap_or(PowerReading::unavailable(PowerAvailability::Fault)),
        });
        select(Timer::after(poll_interval), pause.requested()).await;
    }
}
