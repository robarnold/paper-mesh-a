//! Verify rail sequencing independently of UART and GPIO hardware.
//!
//! # Design choices
//!
//! - Suspend futures at preparation and power-cut boundaries to test cancellation ordering.
//!
//! See [auxiliary rail ownership](../../../../docs/boards/lilygo-t5-s3-pro-power.md#auxiliary-rail).

use super::*;
use core::{
    future::pending,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct State {
    events: Vec<&'static str>,
    fail_enable: bool,
    fail_disable: bool,
    fail_prepare: bool,
    block_prepare: bool,
    block_disable: bool,
}
struct TestRail(Rc<RefCell<State>>);
struct Consumer(Rc<RefCell<State>>);
impl Rail for TestRail {
    type Error = &'static str;
    async fn enable(&mut self) -> Result<(), Self::Error> {
        let mut state = self.0.borrow_mut();
        state.events.push("enable");
        if state.fail_enable {
            Err("enable")
        } else {
            Ok(())
        }
    }
    async fn disable(&mut self) -> Result<(), Self::Error> {
        let (block, fail) = {
            let mut state = self.0.borrow_mut();
            state.events.push("disable");
            (state.block_disable, state.fail_disable)
        };
        if block {
            pending::<()>().await;
        }
        if fail { Err("disable") } else { Ok(()) }
    }
}
impl SuspendParticipant for Consumer {
    type Error = &'static str;
    async fn will_suspend(&mut self, _: u64) -> Result<(), Self::Error> {
        let (block, fail) = {
            let mut state = self.0.borrow_mut();
            state.events.push("prepare");
            (state.block_prepare, state.fail_prepare)
        };
        if block {
            pending::<()>().await;
        }
        if fail { Err("prepare") } else { Ok(()) }
    }
    async fn did_resume(&mut self, _: u64) -> Result<(), Self::Error> {
        self.0.borrow_mut().events.push("resume");
        Ok(())
    }
    async fn did_cancel_suspend(&mut self, _: u64) -> Result<(), Self::Error> {
        self.0.borrow_mut().events.push("cancel");
        Ok(())
    }
}
impl PowerOffParticipant for Consumer {
    type Error = &'static str;
    async fn will_power_off(&mut self, _: u64) -> Result<(), Self::Error> {
        self.0.borrow_mut().events.push("power_off");
        if self.0.borrow().fail_prepare {
            Err("prepare")
        } else {
            Ok(())
        }
    }
}
fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn complete<F: Future>(future: F) -> F::Output {
    match poll(core::pin::pin!(future)) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("unexpected wait"),
    }
}
fn setup() -> (Coordinator<TestRail, Consumer>, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State::default()));
    (
        Coordinator::new(
            TestRail(state.clone()),
            Consumer(state.clone()),
            Box::leak(Box::new(paper_mesh_platform::worker::Pause::new()))
                .split()
                .0,
        ),
        state,
    )
}
fn running() -> (Coordinator<TestRail, Consumer>, Rc<RefCell<State>>) {
    let (mut coordinator, state) = setup();
    complete(coordinator.did_resume(0)).unwrap();
    state.borrow_mut().events.clear();
    (coordinator, state)
}

// A launched consumer stays blocked through failed startup and uncertain restoration.
#[test]
fn rail_dependency_controls_admission() {
    let state = Rc::new(RefCell::new(State::default()));
    let (control, mut supply) =
        Box::leak(Box::new(paper_mesh_platform::worker::Pause::new())).split();
    let mut coordinator =
        Coordinator::new(TestRail(state.clone()), Consumer(state.clone()), control);
    assert!(poll(core::pin::pin!(supply.checkpoint())).is_pending());
    state.borrow_mut().fail_enable = true;
    assert!(complete(coordinator.did_resume(0)).is_err());
    assert!(poll(core::pin::pin!(supply.checkpoint())).is_pending());
    state.borrow_mut().fail_enable = false;
    complete(coordinator.did_resume(10)).unwrap();
    complete(supply.checkpoint());
    complete(coordinator.will_suspend(100)).unwrap();
    // A stale release notification cannot bypass the closed admission state.
    assert!(poll(core::pin::pin!(supply.checkpoint())).is_pending());
    state.borrow_mut().fail_enable = true;
    assert!(complete(coordinator.did_resume(200)).is_err());
    assert!(poll(core::pin::pin!(supply.checkpoint())).is_pending());
    state.borrow_mut().fail_enable = false;
    complete(coordinator.did_resume(200)).unwrap();
    complete(supply.checkpoint());
}

// Cancellation during preparation leaves admission blocked until explicit restoration.
#[test]
fn canceled_preparation_retains_closed_admission() {
    let state = Rc::new(RefCell::new(State::default()));
    let (control, mut supply) =
        Box::leak(Box::new(paper_mesh_platform::worker::Pause::new())).split();
    let mut coordinator =
        Coordinator::new(TestRail(state.clone()), Consumer(state.clone()), control);
    complete(coordinator.did_resume(0)).unwrap();
    state.borrow_mut().block_prepare = true;
    assert!(poll(core::pin::pin!(coordinator.will_suspend(100))).is_pending());
    assert!(poll(core::pin::pin!(supply.checkpoint())).is_pending());
    complete(coordinator.did_cancel_suspend(200)).unwrap();
    complete(supply.checkpoint());
}

// Failed initial enable propagates the fault without restoring consumers or releasing admission.
#[test]
fn startup_failure_keeps_consumers_stopped() {
    let (mut coordinator, state) = setup();
    state.borrow_mut().fail_enable = true;
    assert_eq!(
        complete(coordinator.did_resume(100)),
        Err(Error::Rail("enable"))
    );
    assert_eq!(state.borrow().events, ["enable"]);
}

// A successful transition cuts power after preparation and restores it before releasing consumers.
#[test]
fn suspend_orders_consumer_and_rail_transitions() {
    let (mut coordinator, state) = running();
    complete(coordinator.will_suspend(100)).unwrap();
    complete(coordinator.did_resume(200)).unwrap();
    assert_eq!(
        state.borrow().events,
        ["prepare", "disable", "enable", "resume"]
    );
}

// A preparation failure must not cut the rail, and cancellation can release the partial pause.
#[test]
fn preparation_failure_keeps_power_available() {
    let (mut coordinator, state) = running();
    state.borrow_mut().fail_prepare = true;
    assert_eq!(
        complete(coordinator.will_suspend(100)),
        Err(Error::Consumer("prepare"))
    );
    complete(coordinator.did_cancel_suspend(200)).unwrap();
    assert_eq!(state.borrow().events, ["prepare", "cancel"]);
}

// Canceling preparation before acknowledgement must not attempt a power cut.
#[test]
fn cancellation_before_quiescence_keeps_power_available() {
    let (mut coordinator, state) = running();
    state.borrow_mut().block_prepare = true;
    let mut prepare = Box::pin(coordinator.will_suspend(100));
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    complete(coordinator.did_cancel_suspend(200)).unwrap();
    assert_eq!(state.borrow().events, ["prepare", "cancel"]);
}

// A canceled power-cut future leaves restoration required before consumer release.
#[test]
fn cancellation_during_cut_restores_power_first() {
    let (mut coordinator, state) = running();
    state.borrow_mut().block_disable = true;
    let mut prepare = Box::pin(coordinator.will_suspend(100));
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    complete(coordinator.did_cancel_suspend(200)).unwrap();
    assert_eq!(
        state.borrow().events,
        ["prepare", "disable", "enable", "cancel"]
    );
}

// A cut error has uncertain hardware state. Failed restoration must retain consumer admission blocking.
#[test]
fn cut_and_restore_failures_do_not_release_consumers() {
    let (mut coordinator, state) = running();
    state.borrow_mut().fail_disable = true;
    assert_eq!(
        complete(coordinator.will_suspend(100)),
        Err(Error::Rail("disable"))
    );
    state.borrow_mut().fail_enable = true;
    assert_eq!(
        complete(coordinator.did_cancel_suspend(200)),
        Err(Error::Rail("enable"))
    );
    assert_eq!(state.borrow().events, ["prepare", "disable", "enable"]);
    state.borrow_mut().fail_enable = false;
    complete(coordinator.did_cancel_suspend(300)).unwrap();
    assert_eq!(
        state.borrow().events,
        ["prepare", "disable", "enable", "enable", "cancel"]
    );
}

// Power-off cuts the rail only after successful consumer preparation and never releases consumers.
#[test]
fn power_off_preserves_preparation_order() {
    for fail in [false, true] {
        let (mut coordinator, state) = running();
        state.borrow_mut().fail_prepare = fail;
        assert_eq!(complete(coordinator.will_power_off(100)).is_err(), fail);
        if fail {
            assert_eq!(state.borrow().events, ["power_off"]);
        } else {
            assert_eq!(state.borrow().events, ["power_off", "disable"]);
        }
    }
}
