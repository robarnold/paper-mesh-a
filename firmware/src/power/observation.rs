//! Board composition for the portable power observation service.
//!
//! # Design choices
//!
//! - Supply the board's chip drivers, shared bus, and polling interval to the service.
//! - Keep service storage in firmware while the crate owns its state and coordination.
//!
//! See [power observation ownership](../../../docs/adapters/power-observation.md).

use crate::bus::Bus;
use embassy_time::{Duration, Instant, Timer};
use paper_mesh_platform::power::PowerSnapshot;
use paper_mesh_power_observation::{Lifecycle, Observer, Service, Workers};

pub static SERVICE: Service = Service::new();

/// Supply fresh chip capabilities for a direct sample after lifecycle preparation.
pub async fn read_now(bus: &Bus) -> PowerSnapshot {
    SERVICE
        .read_now(
            &mut Observer::new(
                paper_mesh_bq27220::Bq27220::new(bus),
                paper_mesh_bq25896::observation::Observer::new(bus),
            ),
            || Instant::now().as_millis(),
        )
        .await
}

/// Prepare the service with initialized device resources and the board's polling interval.
pub fn prepare(bus: &'static Bus, poll_interval: Duration) -> (Lifecycle, Workers) {
    assert!(poll_interval.as_ticks() > 0);
    SERVICE.prepare(
        Observer::new(
            paper_mesh_bq27220::Bq27220::new(bus),
            paper_mesh_bq25896::observation::Observer::new(bus),
        ),
        || Instant::now().as_millis(),
        move || Timer::after(poll_interval),
    )
}
