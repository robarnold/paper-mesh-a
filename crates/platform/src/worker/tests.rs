//! Poll pause endpoints directly to exercise cancellation and notification ordering.

extern crate std;

use super::Pause;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

// A pause request cannot complete until the worker reaches its safe checkpoint.
#[test]
fn pause_waits_for_worker_and_resume_releases_it() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(poll(prepare.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(worker.requested())).is_ready());
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(prepare.as_mut()).is_ready());
    drop(prepare);
    control.resume();
    assert!(poll(checkpoint.as_mut()).is_ready());
}

// Acknowledgement before the controller waits must not be lost.
#[test]
fn worker_can_acknowledge_before_controller_waits() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    control.request_pause();
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(control.pause())).is_ready());
    control.resume();
    assert!(poll(checkpoint.as_mut()).is_ready());
}

// Timing out preparation must retain its request until lifecycle cancellation releases it.
#[test]
fn dropping_preparation_keeps_the_request_until_resume() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    assert!(poll(core::pin::pin!(worker.requested())).is_ready());
    control.resume();
    assert!(poll(core::pin::pin!(worker.checkpoint())).is_ready());
    assert!(poll(core::pin::pin!(worker.requested())).is_pending());
}

// Cancellation after acknowledgement must leave the worker parked until explicit release.
#[test]
fn dropping_preparation_after_acknowledgement_does_not_resume_work() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(poll(prepare.as_mut()).is_pending());
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    drop(prepare);
    assert!(poll(checkpoint.as_mut()).is_pending());
    control.resume();
    assert!(poll(checkpoint.as_mut()).is_ready());
}

// A rapid release and new request must not accept an acknowledgement from the earlier pause.
#[test]
fn repeated_pause_requires_a_new_acknowledgement() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    control.request_pause();
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(control.pause())).is_ready());
    control.resume();
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(poll(prepare.as_mut()).is_pending());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(prepare.as_mut()).is_ready());
    drop(prepare);
    control.resume();
    assert!(poll(checkpoint.as_mut()).is_ready());
}

// Dropping a checkpoint revokes its acknowledgement until the worker reaches a new checkpoint.
#[test]
fn canceled_checkpoint_cannot_leave_a_false_acknowledgement() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    control.request_pause();
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    drop(checkpoint);
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(poll(prepare.as_mut()).is_pending());
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(prepare.as_mut()).is_ready());
}

// Separate channels must not acknowledge or stop each other's workers.
#[test]
fn independent_workers_do_not_share_pause_state() {
    let mut first = Pause::<NoopRawMutex>::new();
    let mut second = Pause::<NoopRawMutex>::new();
    let (mut first_control, mut first_worker) = first.split();
    let (mut second_control, mut second_worker) = second.split();
    first_control.request_pause();
    assert!(poll(core::pin::pin!(second_worker.checkpoint())).is_ready());
    let mut first_checkpoint = std::boxed::Box::pin(first_worker.checkpoint());
    assert!(poll(first_checkpoint.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(first_control.pause())).is_ready());
    assert!(poll(core::pin::pin!(second_control.pause())).is_pending());
}

// An idle timer winning its race must not consume a later pause request.
#[test]
fn canceling_an_idle_wait_does_not_lose_a_later_request() {
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    let mut idle = std::boxed::Box::pin(worker.requested());
    assert!(poll(idle.as_mut()).is_pending());
    drop(idle);
    control.request_pause();
    assert!(poll(core::pin::pin!(worker.requested())).is_ready());
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(control.pause())).is_ready());
    control.resume();
    assert!(poll(checkpoint.as_mut()).is_ready());
}

// Requests, acknowledgements, and releases must wake their waiting endpoint without timer polling.
#[test]
fn notifications_wake_both_endpoints() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::Wake;
    struct Counter(AtomicUsize);
    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let worker_wakes = Arc::new(Counter(AtomicUsize::new(0)));
    let control_wakes = Arc::new(Counter(AtomicUsize::new(0)));
    let worker_waker = Waker::from(worker_wakes.clone());
    let control_waker = Waker::from(control_wakes.clone());
    let mut channel = Pause::<NoopRawMutex>::new();
    let (mut control, mut worker) = channel.split();
    let mut request = std::boxed::Box::pin(worker.requested());
    assert!(
        request
            .as_mut()
            .poll(&mut Context::from_waker(&worker_waker))
            .is_pending()
    );
    let mut prepare = std::boxed::Box::pin(control.pause());
    assert!(
        prepare
            .as_mut()
            .poll(&mut Context::from_waker(&control_waker))
            .is_pending()
    );
    assert!(worker_wakes.0.load(Ordering::Relaxed) > 0);
    assert!(poll(request.as_mut()).is_ready());
    drop(request);
    let mut checkpoint = std::boxed::Box::pin(worker.checkpoint());
    assert!(
        checkpoint
            .as_mut()
            .poll(&mut Context::from_waker(&worker_waker))
            .is_pending()
    );
    assert!(control_wakes.0.load(Ordering::Relaxed) > 0);
    assert!(poll(prepare.as_mut()).is_ready());
    drop(prepare);
    let wakes_before_release = worker_wakes.0.load(Ordering::Relaxed);
    control.resume();
    assert!(worker_wakes.0.load(Ordering::Relaxed) > wakes_before_release);
    assert!(poll(checkpoint.as_mut()).is_ready());
}
