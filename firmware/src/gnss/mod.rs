//! Portable GNSS decoding, startup probing, and location publication.
//!
//! # Design choices
//!
//! - Receive bytes through platform serial sessions without owning UART hardware or power transitions.
//!
//! See [GNSS integration](../../../docs/boards/lilygo-t5-s3-pro-gnss.md).

mod config;
pub use config::Config;
pub mod acquisition;
mod gsv;
mod nmea;
pub mod probe;
pub(crate) mod receiver;

use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use paper_mesh_platform::location::{Location, LocationAvailability, LocationSnapshot};

static STATE: Mutex<CriticalSectionRawMutex, RefCell<LocationSnapshot>> =
    Mutex::new(RefCell::new(LocationSnapshot::new()));
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Read-only access to the latest GNSS observations.
pub struct FirmwareLocation;

impl Location for FirmwareLocation {
    fn snapshot(&mut self, _now_ms: u64) -> LocationSnapshot {
        STATE.lock(|state| *state.borrow())
    }
}

pub async fn changed() {
    CHANGED.wait().await;
}

pub(crate) fn publish(snapshot: LocationSnapshot) {
    STATE.lock(|state| *state.borrow_mut() = snapshot);
    CHANGED.signal(());
}

pub fn fault() {
    publish(LocationSnapshot {
        availability: LocationAvailability::Fault,
        ..LocationSnapshot::new()
    });
}
