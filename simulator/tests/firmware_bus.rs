//! Exercise bus contention between interrupt tasks and the synchronous display client.

#[path = "../../firmware/src/bus/arbitration.rs"]
mod arbitration;

use arbitration::Arbitrated;
use core::{
    future::Future,
    task::{Context, Poll},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Wake, Waker};

#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn waker() -> (Arc<Wakes>, Waker) {
    let count = Arc::new(Wakes::default());
    (count.clone(), Waker::from(count))
}

#[test]
fn contending_clients_wait_quietly_and_get_the_bus_in_order() {
    let bus = Arbitrated::<NoopRawMutex, _, 8>::new(0);
    let owner = bus.try_lock().unwrap();
    let (first_wakes, first_waker) = waker();
    let (second_wakes, second_waker) = waker();
    let mut first = Box::pin(bus.lock());
    let mut second = Box::pin(bus.lock());
    let mut first_context = Context::from_waker(&first_waker);
    let mut second_context = Context::from_waker(&second_waker);
    for _ in 0..100 {
        assert!(first.as_mut().poll(&mut first_context).is_pending());
        assert!(second.as_mut().poll(&mut second_context).is_pending());
        assert!(bus.lock_blocking(|| false).is_none());
    }
    assert_eq!(first_wakes.0.load(Ordering::Relaxed), 0);
    assert_eq!(second_wakes.0.load(Ordering::Relaxed), 0);
    drop(owner);
    assert_eq!(first_wakes.0.load(Ordering::Relaxed), 1);
    assert_eq!(second_wakes.0.load(Ordering::Relaxed), 0);
    assert!(bus.try_lock().is_none());
    let Poll::Ready(mut first_guard) = first.as_mut().poll(&mut first_context) else {
        panic!("first client was not ready")
    };
    *first_guard = 42;
    drop(first_guard);
    assert_eq!(second_wakes.0.load(Ordering::Relaxed), 1);
    let Poll::Ready(second_guard) = second.as_mut().poll(&mut second_context) else {
        panic!("second client was not ready")
    };
    assert_eq!(*second_guard, 42);
    drop(second_guard);
    assert_eq!(*bus.lock_blocking(|| false).unwrap(), 42);
}

#[test]
fn canceling_a_waiter_releases_its_queue_slot_and_preserves_the_next_client() {
    let bus = Arbitrated::<NoopRawMutex, _, 2>::new(7);
    let owner = bus.try_lock().unwrap();
    let (_, first_waker) = waker();
    let (second_wakes, second_waker) = waker();
    let mut first = Box::pin(bus.lock());
    let mut second = Box::pin(bus.lock());
    assert!(
        first
            .as_mut()
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending()
    );
    assert!(
        second
            .as_mut()
            .poll(&mut Context::from_waker(&second_waker))
            .is_pending()
    );
    drop(first);
    let (_, third_waker) = waker();
    let mut third = Box::pin(bus.lock());
    assert!(
        third
            .as_mut()
            .poll(&mut Context::from_waker(&third_waker))
            .is_pending()
    );
    drop(owner);
    assert_eq!(second_wakes.0.load(Ordering::Relaxed), 1);
    let Poll::Ready(guard) = second
        .as_mut()
        .poll(&mut Context::from_waker(&second_waker))
    else {
        panic!("second client was not ready")
    };
    drop(guard);
    assert!(
        third
            .as_mut()
            .poll(&mut Context::from_waker(&third_waker))
            .is_ready()
    );
}

#[test]
fn synchronous_wait_stops_at_its_retry_budget_or_acquires_after_release() {
    let bus = Arbitrated::<NoopRawMutex, _, 2>::new(3);
    let mut owner = Some(bus.try_lock().unwrap());
    let mut remaining = 3;
    assert!(
        bus.lock_blocking(|| {
            remaining -= 1;
            remaining > 0
        })
        .is_none()
    );
    assert_eq!(remaining, 0);
    let guard = bus
        .lock_blocking(|| {
            owner.take();
            true
        })
        .unwrap();
    assert_eq!(*guard, 3);
}
