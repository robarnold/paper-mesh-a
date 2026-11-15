#![no_std]

//! Portable battery and charger observation sampling and worker coordination.
//!
//! # Design choices
//!
//! - Sample sources independently so one device failure preserves the other observation.
//! - Finish both reads and publication before acknowledging a pause.
//! - Own cache publication, lifecycle coordination, and task storage while the runtime supplies devices, timing, and executor placement.
//!
//! See [power observations](../../../docs/adapters/power-observation.md).

extern crate alloc;

mod lifecycle;
mod service;
pub use lifecycle::Lifecycle;
pub use service::Service;
mod workers;
pub use workers::Workers;

use alloc::boxed::Box;
use core::{fmt::Debug, future::Future};
use embassy_futures::select::select;
use embassy_sync::blocking_mutex::raw::RawMutex;
use paper_mesh_platform::{
    power::{
        BatteryGauge, BatteryState, ChargerObserver, ChargerState, PowerAvailability, PowerReading,
        PowerSnapshot,
    },
    worker::PauseWorker,
};

/// Device capabilities used to sample battery and charger state independently.
pub struct Observer<G, C> {
    gauge: G,
    charger: C,
}

struct Sample<G, C> {
    battery: Result<PowerReading<BatteryState>, G>,
    charger: Result<PowerReading<ChargerState>, C>,
}

impl<G, C> Sample<G, C> {
    fn snapshot(self) -> PowerSnapshot {
        PowerSnapshot {
            battery: self
                .battery
                .unwrap_or(PowerReading::unavailable(PowerAvailability::Fault)),
            charger: self
                .charger
                .unwrap_or(PowerReading::unavailable(PowerAvailability::Fault)),
        }
    }
}

impl<G: BatteryGauge, C: ChargerObserver> Observer<G, C> {
    /// Take device capabilities without reading or changing either device.
    pub const fn new(gauge: G, charger: C) -> Self {
        Self { gauge, charger }
    }

    async fn sample(&mut self, now: impl Fn() -> u64) -> Sample<G::Error, C::Error> {
        let battery = self
            .gauge
            .read()
            .await
            .map(|value| PowerReading::ready(value, now()));
        let charger = self
            .charger
            .read()
            .await
            .map(|value| PowerReading::ready(value, now()));
        Sample { battery, charger }
    }

    /// Read both sources, timestamping each success with monotonic milliseconds after completion.
    /// A failed source becomes unavailable without discarding the other source's result.
    /// Await completion before claiming quiescence. Cancellation inherits each device's read contract.
    pub async fn read(&mut self, now: impl Fn() -> u64) -> PowerSnapshot {
        self.sample(now).await.snapshot()
    }

    /// Prepare one task without starting observations. The runtime supplies its executor.
    /// Allocate the worker future once. Device reads and polling cycles add no worker allocations.
    fn prepare<M: RawMutex + 'static, F: Future<Output = ()> + 'static>(
        self,
        mut pause: PauseWorker<'static, M>,
        now: impl Fn() -> u64 + 'static,
        delay: impl FnMut() -> F + 'static,
        publish: impl FnMut(PowerSnapshot) + 'static,
    ) -> Workers
    where
        G: 'static,
        C: 'static,
        G::Error: Debug,
        C::Error: Debug,
    {
        Workers {
            future: Box::pin(async move {
                self.run(&mut pause, now, delay, publish).await;
            }),
        }
    }

    /// Sample and publish until canceled. Runtime delay selects the polling interval.
    /// Pause requests interrupt the delay, but never a read or publication already in progress.
    /// Runtime delay futures must tolerate cancellation. Dropping this worker does not establish quiescence.
    pub async fn run<M: RawMutex, F: Future<Output = ()>>(
        mut self,
        pause: &mut PauseWorker<'_, M>,
        now: impl Fn() -> u64,
        mut delay: impl FnMut() -> F,
        mut publish: impl FnMut(PowerSnapshot),
    ) where
        G::Error: Debug,
        C::Error: Debug,
    {
        let mut failed = (false, false);
        loop {
            pause.checkpoint().await;
            let sample = self.sample(&now).await;
            if let Err(error) = &sample.battery
                && !failed.0
            {
                log::error!("battery: read failed: {:?}", error);
            }
            if let Err(error) = &sample.charger
                && !failed.1
            {
                log::error!("charger: read failed: {:?}", error);
            }
            failed = (sample.battery.is_err(), sample.charger.is_err());
            publish(sample.snapshot());
            select(delay(), pause.requested()).await;
        }
    }
}
