//! Exercise preparation failures and cancellation without UART hardware.

use super::*;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use paper_mesh_platform::worker::{Pause, PauseWorker};

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn setup() -> (
    GnssLifecycle,
    PauseWorker<'static, PauseMutex>,
    &'static PreparationReport,
) {
    let channel = Box::leak(Box::new(Pause::new()));
    let (pause, worker) = channel.split();
    let report = Box::leak(Box::new(Mutex::new(Cell::new(Ok(())))));
    (GnssLifecycle { pause, report }, worker, report)
}

// Hardware results become visible only after acknowledgement, including both failure causes.
#[test]
fn preparation_waits_for_acknowledgement_and_preserves_failures() {
    for (uart, supply) in [(true, true), (false, true), (true, false), (false, false)] {
        let (mut lifecycle, mut worker, report) = setup();
        lifecycle.request_pause();
        let mut prepare = Box::pin(lifecycle.will_suspend(0));
        assert!(poll(prepare.as_mut()).is_pending());
        publish_preparation(report, uart, supply);
        assert!(poll(prepare.as_mut()).is_pending());
        let mut checkpoint = Box::pin(worker.checkpoint());
        assert!(poll(checkpoint.as_mut()).is_pending());
        let expected = if uart && supply {
            Ok(())
        } else {
            Err(PreparationError {
                idle_uart_failed: !uart,
                supply_shutdown_failed: !supply,
            })
        };
        assert_eq!(poll(prepare.as_mut()), Poll::Ready(expected));
        drop(prepare);
        assert_eq!(
            poll(core::pin::pin!(lifecycle.did_cancel_suspend(0))),
            Poll::Ready(Ok(()))
        );
        assert!(poll(checkpoint.as_mut()).is_ready());
    }
}

// A retry must wait for a new acknowledgement and replace the previous failure.
#[test]
fn retry_uses_the_new_preparation_result() {
    let (mut lifecycle, mut worker, report) = setup();
    for ready in [false, true] {
        let mut prepare = Box::pin(lifecycle.will_suspend(0));
        assert!(poll(prepare.as_mut()).is_pending());
        publish_preparation(report, ready, ready);
        let mut checkpoint = Box::pin(worker.checkpoint());
        assert!(poll(checkpoint.as_mut()).is_pending());
        assert_eq!(poll(prepare.as_mut()), Poll::Ready(report.lock(Cell::get)));
        drop(prepare);
        assert_eq!(
            poll(core::pin::pin!(lifecycle.did_resume(0))),
            Poll::Ready(Ok(()))
        );
        assert!(poll(checkpoint.as_mut()).is_ready());
    }
}

// Cancellation releases a request even when hardware preparation has not acknowledged it.
#[test]
fn cancellation_releases_incomplete_preparation() {
    let (mut lifecycle, mut worker, _) = setup();
    let mut prepare = Box::pin(lifecycle.will_suspend(0));
    assert!(poll(prepare.as_mut()).is_pending());
    drop(prepare);
    assert!(worker.is_requested());
    assert_eq!(
        poll(core::pin::pin!(lifecycle.did_cancel_suspend(0))),
        Poll::Ready(Ok(()))
    );
    assert!(!worker.is_requested());
    assert!(poll(core::pin::pin!(worker.checkpoint())).is_ready());
}

// Failed power-off preparation keeps the worker stopped for reset recovery.
#[test]
fn power_off_failure_retains_the_pause() {
    let (mut lifecycle, mut worker, report) = setup();
    let mut prepare = Box::pin(lifecycle.will_power_off(0));
    assert!(poll(prepare.as_mut()).is_pending());
    publish_preparation(report, true, false);
    let mut checkpoint = Box::pin(worker.checkpoint());
    assert!(poll(checkpoint.as_mut()).is_pending());
    assert_eq!(poll(prepare.as_mut()), Poll::Ready(report.lock(Cell::get)));
    drop(prepare);
    assert!(poll(checkpoint.as_mut()).is_pending());
}
