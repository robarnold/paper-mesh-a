//! Periodic power observations on core 0 through the shared I2C bus.
//!
//! # Design choices
//!
//! - Publish gauge and charger observations independently so a bus error for one
//!   device does not invalidate the other.
//!
//! See [Hardware and execution](../../../docs/boards/lilygo-t5-s3-pro-power.md#hardware-and-execution).

mod device;
mod observation_lifecycle;
pub use observation_lifecycle::PowerObservationLifecycle;
pub(crate) mod ship;

use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use paper_mesh_platform::power::{Power, PowerAvailability, PowerReading, PowerSnapshot};
use paper_mesh_platform::worker::{Pause, PauseWorker};
use static_cell::StaticCell;

use crate::bus::Bus;

const POLL_MS: u64 = 2_000;
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

pub(crate) fn ship_error(
    error: ship::Error<esp_hal::i2c::master::Error>,
) -> paper_mesh_platform::power_off::PowerOffError {
    match error {
        ship::Error::ExternalPower => paper_mesh_platform::power_off::PowerOffError::ExternalPower,
        ship::Error::Bus(_) | ship::Error::Verification => {
            paper_mesh_platform::power_off::PowerOffError::Preparation
        }
    }
}

pub async fn check_power_off(
    bus: &Bus,
) -> Result<(), paper_mesh_platform::power_off::PowerOffError> {
    ship::check(&mut *bus.lock().await).map_err(ship_error)
}

pub fn fault() {
    publish(PowerSnapshot {
        battery: PowerReading::unavailable(PowerAvailability::Fault),
        charger: PowerReading::unavailable(PowerAvailability::Fault),
    });
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

pub fn start_observation(spawner: Spawner, bus: &'static Bus) -> PowerObservationLifecycle {
    let (pause, worker_pause) = PAUSE.init(Pause::new()).split();
    spawner.spawn(observation_worker(bus, worker_pause).unwrap());
    PowerObservationLifecycle { pause }
}

#[embassy_executor::task]
async fn observation_worker(
    bus: &'static Bus,
    mut pause: PauseWorker<'static, CriticalSectionRawMutex>,
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
        select(Timer::after_millis(POLL_MS), pause.requested()).await;
    }
}
