//! Verify readiness cancellation and retained wake capture without GPIO hardware.
//!
//! # Design choices
//!
//! - Drive the private provider state with fake pin latches to exercise event ordering deterministically.
//!
//! See [GPIO readiness provider](../../../../../docs/adapters/gpio-readiness.md).

use super::*;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use std::{boxed::Box, rc::Rc};

#[derive(Default)]
struct Pins {
    active: bool,
    pending: bool,
    enabled: bool,
    activate_on_enable: bool,
}
#[derive(Clone)]
struct Pin(Rc<RefCell<Pins>>);
impl Input for Pin {
    fn active(&self) -> bool {
        self.0.borrow().active
    }
    fn pending(&self) -> bool {
        self.0.borrow().pending
    }
    fn mask(&mut self) {
        self.0.borrow_mut().enabled = false;
    }
    fn clear(&mut self) {
        self.0.borrow_mut().pending = false;
    }
    fn enable(&mut self) {
        let mut pins = self.0.borrow_mut();
        pins.enabled = true;
        if pins.activate_on_enable {
            pins.active = true;
            pins.pending = true;
        }
    }
}
fn provider() -> (State<NoopRawMutex, Pin>, Rc<RefCell<Pins>>) {
    let pin = Pin(Rc::new(RefCell::new(Pins::default())));
    (State::new(pin.clone()), pin.0)
}
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

// Already-active input must request a read without a new GPIO edge.
#[test]
fn active_input_completes_without_an_edge() {
    let (state, pin) = provider();
    pin.borrow_mut().active = true;
    state.start_capture(12);
    for _ in 0..2 {
        let mut wait = Box::pin(state.wait_for_readiness(|| 12));
        assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 12 }));
        assert!(!pin.borrow().enabled);
    }
}

// Canceling delivery must retain capture, both before and after an interrupt arrives.
#[test]
fn cancellation_preserves_capture_and_pending_readiness() {
    for before in [true, false] {
        let (state, pin) = provider();
        state.start_capture(0);
        let mut wait = Box::pin(state.wait_for_readiness(|| 0));
        assert!(poll(wait.as_mut()).is_pending());
        assert_eq!(state.arm_wake_capture(0), Err(ArmError::ReaderActive));
        if before {
            pin.borrow_mut().pending = true;
            state.did_interrupt(10);
        }
        drop(wait);
        if !before {
            assert!(pin.borrow().enabled);
            pin.borrow_mut().pending = true;
            state.did_interrupt(10);
        }
        let mut next = Box::pin(state.wait_for_readiness(|| 20));
        assert_eq!(poll(next.as_mut()), Poll::Ready(Ready { at_us: 10 }));
        assert!(pin.borrow().enabled);
    }
}

// A level becoming active during enable must remain observable.
#[test]
fn activation_during_enable_is_not_lost() {
    let (state, pin) = provider();
    pin.borrow_mut().activate_on_enable = true;
    state.start_capture(0);
    state.did_interrupt(25);
    let mut wait = Box::pin(state.wait_for_readiness(|| 30));
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 25 }));
}

// Notifications coalesce while no reader waits and retain the first observation time.
#[test]
fn capture_coalesces_without_a_reader() {
    let (state, pin) = provider();
    state.start_capture(0);
    assert!(!state.did_interrupt(1));
    for at in [2, 3] {
        pin.borrow_mut().pending = true;
        assert!(!state.did_interrupt(at));
    }
    let mut wait = Box::pin(state.wait_for_readiness(|| 4));
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 2 }));
    drop(wait);
    assert!(pin.borrow().enabled);
    pin.borrow_mut().pending = true;
    state.did_interrupt(5);
    let mut next = Box::pin(state.wait_for_readiness(|| 6));
    assert_eq!(poll(next.as_mut()), Poll::Ready(Ready { at_us: 5 }));
}

// Wake capture survives acknowledgement, completion, and ordinary readiness delivery.
#[test]
fn wake_capture_survives_finish_and_delivery() {
    let (state, pin) = provider();
    state.start_capture(0);
    assert_eq!(state.arm_wake_capture(1), Ok(()));
    pin.borrow_mut().pending = true;
    assert!(state.did_interrupt(10));
    assert!(!pin.borrow().pending);
    assert_eq!(state.finish_wake_capture(11), Some(WakeCapture::Captured));
    assert_eq!(state.finish_wake_capture(12), None);
    let mut wait = Box::pin(state.wait_for_readiness(|| 20));
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 10 }));
    assert_eq!(
        state.take_restoration_observation(),
        RestorationObservation {
            wake: WakeCapture::Captured,
            input_active: false
        }
    );
    assert_eq!(
        state.take_restoration_observation(),
        RestorationObservation {
            wake: WakeCapture::NotCaptured,
            input_active: false
        }
    );
}

// Wake completion captures a latch even when its callback has not run.
#[test]
fn wake_finish_captures_unhandled_latch() {
    let (state, pin) = provider();
    state.start_capture(0);
    assert_eq!(state.arm_wake_capture(1), Ok(()));
    pin.borrow_mut().pending = true;
    assert_eq!(state.finish_wake_capture(10), Some(WakeCapture::Captured));
    assert_eq!(
        state.take_restoration_observation(),
        RestorationObservation {
            wake: WakeCapture::Captured,
            input_active: false
        }
    );
    let mut wait = Box::pin(state.wait_for_readiness(|| 20));
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 10 }));
}

// Active input blocks wake admission, and an earlier readiness notification survives an empty wake interval.
#[test]
fn wake_admission_preserves_ordinary_readiness() {
    let (state, pin) = provider();
    state.start_capture(0);
    pin.borrow_mut().active = true;
    assert_eq!(state.arm_wake_capture(1), Err(ArmError::InputActive));
    pin.borrow_mut().active = false;
    pin.borrow_mut().pending = true;
    assert_eq!(state.arm_wake_capture(2), Ok(()));
    assert_eq!(state.finish_wake_capture(3), Some(WakeCapture::NotCaptured));
    let mut wait = Box::pin(state.wait_for_readiness(|| 4));
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 2 }));
}

// One provider's notification must not complete another provider's wait.
#[test]
fn independent_providers_keep_separate_notifications() {
    let (first, pin) = provider();
    let (second, _) = provider();
    first.start_capture(0);
    second.start_capture(0);
    let mut a = Box::pin(first.wait_for_readiness(|| 0));
    let mut b = Box::pin(second.wait_for_readiness(|| 0));
    assert!(poll(a.as_mut()).is_pending());
    assert!(poll(b.as_mut()).is_pending());
    pin.borrow_mut().pending = true;
    first.did_interrupt(8);
    assert_eq!(poll(a.as_mut()), Poll::Ready(Ready { at_us: 8 }));
    assert!(poll(b.as_mut()).is_pending());
}

// Wake admission outside ordinary capture must refuse without ending an existing wake interval.
#[test]
fn wake_admission_requires_ordinary_capture() {
    let (state, _) = provider();
    assert_eq!(state.arm_wake_capture(0), Err(ArmError::NotCapturing));
    state.start_capture(1);
    assert_eq!(state.arm_wake_capture(2), Ok(()));
    assert_eq!(state.arm_wake_capture(3), Err(ArmError::NotCapturing));
    assert_eq!(state.finish_wake_capture(4), Some(WakeCapture::NotCaptured));
}

// Consuming wake evidence must not hide a level that remains active across restoration observations.
#[test]
fn restoration_observation_keeps_level_separate_from_wake() {
    let (state, pin) = provider();
    state.start_capture(0);
    assert_eq!(state.arm_wake_capture(1), Ok(()));
    pin.borrow_mut().pending = true;
    pin.borrow_mut().active = true;
    state.did_interrupt(2);
    assert_eq!(state.finish_wake_capture(3), Some(WakeCapture::Captured));
    assert_eq!(
        state.take_restoration_observation(),
        RestorationObservation {
            wake: WakeCapture::Captured,
            input_active: true,
        }
    );
    assert_eq!(
        state.take_restoration_observation(),
        RestorationObservation {
            wake: WakeCapture::NotCaptured,
            input_active: true,
        }
    );
}
