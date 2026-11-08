//! Exercise USB pause ordering without endpoints or an executor.
//!
//! # Design choices
//!
//! - Poll worker checkpoints separately to expose premature device detachment.
//! - Substitute session callbacks so cancellation can verify connection invalidation.
//!
//! See [USB coordination](../../docs/platform/worker-coordination.md#usb-integration).

#[allow(dead_code)]
#[path = "../../firmware/src/usb/lifecycle.rs"]
mod lifecycle;

use core::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use lifecycle::{UsbLifecycle, Worker};
use paper_mesh_platform::{
    power_off::PowerOffParticipant, suspend::SuspendParticipant, worker::Pause,
};

thread_local! {
    static INVALIDATIONS: Cell<usize> = const { Cell::new(0) };
    static RESTARTS: Cell<usize> = const { Cell::new(0) };
}

fn change(status: u32) {
    assert_eq!(status, 0);
    INVALIDATIONS.set(INVALIDATIONS.get() + 1);
}

struct Restart;
static RESTART: Restart = Restart;
impl Restart {
    fn signal(&self, (): ()) {
        RESTARTS.set(RESTARTS.get() + 1);
    }
}

fn setup() -> (UsbLifecycle, [Worker; 4]) {
    let channels = Box::leak(Box::new(core::array::from_fn::<_, 4, _>(|_| {
        Pause::<NoopRawMutex>::new()
    })));
    let [device, hid, logs, commands] = channels;
    let (device, device_worker) = device.split();
    let (hid, hid_worker) = hid.split();
    let (logs, log_worker) = logs.split();
    let (commands, command_worker) = commands.split();
    (
        UsbLifecycle {
            device,
            hid,
            logs,
            commands,
        },
        [device_worker, hid_worker, log_worker, command_worker],
    )
}

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

// Device preparation must wait for every endpoint operation to stop.
#[test]
fn endpoint_acknowledgements_precede_device_pause() {
    let (mut usb, [mut device, mut hid, mut logs, mut commands]) = setup();
    let mut prepare = Box::pin(usb.will_suspend(10));
    assert!(poll(prepare.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(device.requested())).is_pending());
    let mut commands = Box::pin(commands.checkpoint());
    let mut hid = Box::pin(hid.checkpoint());
    assert!(poll(commands.as_mut()).is_pending());
    assert!(poll(hid.as_mut()).is_pending());
    assert!(poll(prepare.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(device.requested())).is_pending());
    let mut logs = Box::pin(logs.checkpoint());
    assert!(poll(logs.as_mut()).is_pending());
    assert!(poll(prepare.as_mut()).is_pending());
    assert!(poll(core::pin::pin!(device.requested())).is_ready());
    let mut device = Box::pin(device.checkpoint());
    assert!(poll(device.as_mut()).is_pending());
    assert_eq!(poll(prepare.as_mut()), Poll::Ready(Ok(())));
    drop(prepare);
    let invalidations = INVALIDATIONS.get();
    let restarts = RESTARTS.get();
    assert_eq!(
        poll(core::pin::pin!(usb.did_resume(20))),
        Poll::Ready(Ok(()))
    );
    assert_eq!(INVALIDATIONS.get(), invalidations + 1);
    assert_eq!(RESTARTS.get(), restarts + 1);
    assert!(poll(device.as_mut()).is_ready());
    assert!(poll(hid.as_mut()).is_ready());
    assert!(poll(logs.as_mut()).is_ready());
    assert!(poll(commands.as_mut()).is_ready());
}

// Cancellation must release pending requests and invalidate sessions before detachment has occurred.
#[test]
fn partial_preparation_cancellation_releases_every_worker() {
    let (mut usb, mut workers) = setup();
    let mut prepare = Box::pin(usb.will_suspend(10));
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    let invalidations = INVALIDATIONS.get();
    let restarts = RESTARTS.get();
    assert_eq!(
        poll(core::pin::pin!(usb.did_cancel_suspend(20))),
        Poll::Ready(Ok(()))
    );
    assert_eq!(INVALIDATIONS.get(), invalidations + 1);
    assert_eq!(RESTARTS.get(), restarts + 1);
    for worker in &mut workers {
        assert!(poll(core::pin::pin!(worker.checkpoint())).is_ready());
        assert!(poll(core::pin::pin!(worker.requested())).is_pending());
    }
}

// An aborted power-off preparation must keep requests active until reset.
#[test]
fn power_off_cancellation_does_not_release_workers() {
    let (mut usb, mut workers) = setup();
    let mut prepare = Box::pin(usb.will_power_off(10));
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    for worker in &mut workers[1..] {
        assert!(poll(core::pin::pin!(worker.requested())).is_ready());
    }
    usb.request_pause();
    for worker in &mut workers {
        assert!(poll(core::pin::pin!(worker.requested())).is_ready());
    }
}
