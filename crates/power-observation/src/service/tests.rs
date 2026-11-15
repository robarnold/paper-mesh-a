//! Verify that service handles coordinate the same worker and published state.
//!
//! # Design choices
//!
//! - Poll the prepared future directly to test lifecycle ordering without an executor or timer driver.
//!
//! See [power observation ownership](../../../../docs/adapters/power-observation.md).

use super::*;
use alloc::boxed::Box;
use core::{
    cell::Cell,
    future::{pending, poll_fn},
    task::{Context, Poll, Waker},
};
use paper_mesh_platform::{
    power::{BatteryState, ChargerState, PowerAvailability},
    power_off::PowerOffParticipant,
    suspend::SuspendParticipant,
};

struct Gauge(&'static Cell<bool>);
impl BatteryGauge for Gauge {
    type Error = ();
    async fn read(&mut self) -> Result<BatteryState, ()> {
        poll_fn(|_| {
            if self.0.get() {
                Poll::Ready(Ok(BatteryState::default()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
struct Charger;
impl ChargerObserver for Charger {
    type Error = ();
    async fn read(&mut self) -> Result<ChargerState, ()> {
        Err(())
    }
}

// Preparation, publication, direct reads, restoration, and shutdown share one cache and pause channel.
#[test]
fn handles_coordinate_publication_and_lifecycle() {
    let ready: &'static Cell<bool> = Box::leak(Box::new(Cell::new(false)));
    let now: &'static Cell<u64> = Box::leak(Box::new(Cell::new(17)));
    let service = Box::leak(Box::new(Service::new()));
    assert_eq!((&*service).snapshot(0), PowerSnapshot::INITIAL);
    let mut observations = &*service;
    let (mut lifecycle, mut workers) = service.prepare(
        Observer::new(Gauge(ready), Charger),
        move || now.get(),
        pending,
    );
    let mut cx = Context::from_waker(Waker::noop());
    // A canceled wait must not discard the later publication.
    assert!(
        core::pin::pin!(observations.changed())
            .as_mut()
            .poll(&mut cx)
            .is_pending()
    );
    assert!(workers.future.as_mut().poll(&mut cx).is_pending());
    {
        let mut suspend = core::pin::pin!(lifecycle.will_suspend(100));
        assert!(suspend.as_mut().poll(&mut cx).is_pending());
        assert!(workers.future.as_mut().poll(&mut cx).is_pending());
        assert!(suspend.as_mut().poll(&mut cx).is_pending());
        assert_eq!(observations.snapshot(0), PowerSnapshot::INITIAL);
        ready.set(true);
        assert!(workers.future.as_mut().poll(&mut cx).is_pending());
        assert!(matches!(
            suspend.as_mut().poll(&mut cx),
            Poll::Ready(Ok(()))
        ));
    }
    assert_eq!(
        observations
            .snapshot(0)
            .battery
            .observation
            .unwrap()
            .observed_at_ms,
        17
    );
    assert_eq!(
        observations.snapshot(0).charger.availability,
        PowerAvailability::Fault
    );
    assert!(
        core::pin::pin!(observations.changed())
            .as_mut()
            .poll(&mut cx)
            .is_ready()
    );
    let mut direct = Observer::new(Gauge(ready), Charger);
    assert!(
        core::pin::pin!(service.read_now(&mut direct, || 23))
            .as_mut()
            .poll(&mut cx)
            .is_ready()
    );
    assert_eq!(
        observations
            .snapshot(0)
            .battery
            .observation
            .unwrap()
            .observed_at_ms,
        23
    );
    assert!(
        core::pin::pin!(observations.changed())
            .as_mut()
            .poll(&mut cx)
            .is_ready()
    );
    now.set(24);
    assert!(
        core::pin::pin!(lifecycle.did_cancel_suspend(100))
            .as_mut()
            .poll(&mut cx)
            .is_ready()
    );
    assert!(workers.future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(
        observations
            .snapshot(0)
            .battery
            .observation
            .unwrap()
            .observed_at_ms,
        24
    );
    let mut shutdown = core::pin::pin!(lifecycle.will_power_off(100));
    assert!(shutdown.as_mut().poll(&mut cx).is_pending());
    assert!(workers.future.as_mut().poll(&mut cx).is_pending());
    assert!(matches!(
        shutdown.as_mut().poll(&mut cx),
        Poll::Ready(Ok(()))
    ));
}
