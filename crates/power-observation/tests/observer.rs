//! Verify independent observations and pause acknowledgement without hardware or timers.
//!
//! # Design choices
//!
//! - Control read completion to distinguish a pause request from released device resources.
//!
//! See [power observations](../../../docs/adapters/power-observation.md).

use core::{
    cell::Cell,
    future::{Future, pending, poll_fn},
    task::{Context, Poll, Waker},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use paper_mesh_platform::{
    power::{BatteryGauge, BatteryState, ChargerObserver, ChargerState, PowerAvailability},
    worker::Pause,
};
use paper_mesh_power_observation::Observer;

struct Gauge<'a> {
    ready: &'a Cell<bool>,
    fail: &'a Cell<bool>,
    reads: &'a Cell<u32>,
}
impl BatteryGauge for Gauge<'_> {
    type Error = ();
    async fn read(&mut self) -> Result<BatteryState, ()> {
        self.reads.set(self.reads.get() + 1);
        poll_fn(|_| {
            if self.ready.get() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        if self.fail.get() {
            Err(())
        } else {
            Ok(BatteryState::default())
        }
    }
}
struct Charger(bool);
impl ChargerObserver for Charger {
    type Error = ();
    async fn read(&mut self) -> Result<ChargerState, ()> {
        if self.0 {
            Err(())
        } else {
            Ok(ChargerState {
                source: paper_mesh_platform::power::PowerSource::None,
                charge: paper_mesh_platform::power::ChargeState::Idle,
                power_good: false,
                charge_enabled: false,
                input_suspended: false,
                input_limit_ma: 0,
                charge_limit_ma: 0,
                charge_voltage_mv: 0,
                termination_current_ma: 0,
                input_voltage_limit_mv: 0,
                input_current_limited: false,
                input_voltage_limited: false,
                thermal_regulation: false,
                minimum_system_voltage: false,
                faults: Default::default(),
                latched_faults: Default::default(),
            })
        }
    }
}

// Each successful source gets its completion timestamp, including after recovery from an independent fault.
#[test]
fn sources_fail_independently_and_recover() {
    let ready = Cell::new(true);
    let fail = Cell::new(true);
    let reads = Cell::new(0);
    let now = Cell::new(10);
    let clock = || {
        let at = now.get();
        now.set(at + 1);
        at
    };
    let mut observer = Observer::new(
        Gauge {
            ready: &ready,
            fail: &fail,
            reads: &reads,
        },
        Charger(false),
    );
    let mut cx = Context::from_waker(Waker::noop());
    let Poll::Ready(first) = core::pin::pin!(observer.read(clock)).as_mut().poll(&mut cx) else {
        panic!("read should complete")
    };
    assert_eq!(first.battery.availability, PowerAvailability::Fault);
    assert!(first.battery.observation.is_none());
    assert_eq!(first.charger.observation.unwrap().observed_at_ms, 10);
    fail.set(false);
    let Poll::Ready(second) = core::pin::pin!(observer.read(clock)).as_mut().poll(&mut cx) else {
        panic!("read should complete")
    };
    assert_eq!(second.battery.observation.unwrap().observed_at_ms, 11);
    assert_eq!(second.charger.observation.unwrap().observed_at_ms, 12);
    let mut observer = Observer::new(
        Gauge {
            ready: &ready,
            fail: &fail,
            reads: &reads,
        },
        Charger(true),
    );
    let Poll::Ready(third) = core::pin::pin!(observer.read(clock)).as_mut().poll(&mut cx) else {
        panic!("read should complete")
    };
    assert_eq!(third.battery.availability, PowerAvailability::Ready);
    assert_eq!(third.charger.availability, PowerAvailability::Fault);
}

// A pause during acquisition waits for both reads and publication, then interrupts the pending delay.
#[test]
fn pause_waits_for_publication_and_resume_starts_another_sample() {
    let ready = Cell::new(false);
    let fail = Cell::new(false);
    let reads = Cell::new(0);
    let published = Cell::new(0);
    let mut pause = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = pause.split();
    let observer = Observer::new(
        Gauge {
            ready: &ready,
            fail: &fail,
            reads: &reads,
        },
        Charger(false),
    );
    let mut run = core::pin::pin!(observer.run(
        &mut worker,
        || 7,
        pending,
        |_| published.set(published.get() + 1)
    ));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(reads.get(), 0);
    control.resume();
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(reads.get(), 1);
    {
        let mut prepared = core::pin::pin!(control.pause());
        assert!(prepared.as_mut().poll(&mut cx).is_pending());
        assert!(run.as_mut().poll(&mut cx).is_pending());
        assert!(prepared.as_mut().poll(&mut cx).is_pending());
        assert_eq!(published.get(), 0);
        ready.set(true);
        assert!(run.as_mut().poll(&mut cx).is_pending());
        assert_eq!(published.get(), 1);
        assert!(prepared.as_mut().poll(&mut cx).is_ready());
    }
    control.resume();
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(published.get(), 2);
    assert_eq!(reads.get(), 2);
}
