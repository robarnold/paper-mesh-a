//! Verify acquisition admission and cancellation at the reader boundary.
//!
//! See [acquisition control](../../../../docs/platform/readiness.md#acquisition-control).

extern crate std;

use super::*;
use core::task::{Context, Waker};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use std::{
    boxed::Box,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

struct Source(Arc<AtomicU64>);
impl Readiness for Source {
    async fn wait(&mut self) -> Ready {
        poll_fn(|_| match self.0.swap(0, Ordering::SeqCst) {
            0 => Poll::Pending,
            at_us => Poll::Ready(Ready { at_us }),
        })
        .await
    }
}
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

// Creating a wait cannot acknowledge release of resources without polling the reader.
#[test]
fn unpolled_wait_does_not_acknowledge_quiescence() {
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(Arc::new(AtomicU64::new(0))));
    let mut wait = Box::pin(reader.wait());
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    assert!(poll(wait.as_mut()).is_pending());
    assert!(poll(stop.as_mut()).is_ready());
}

// An idle reader must acknowledge a request and retain readiness until restoration releases it.
#[test]
fn waiting_reader_blocks_delivery_until_resume() {
    let source = Arc::new(AtomicU64::new(0));
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(source.clone()));
    let mut wait = Box::pin(reader.wait());
    assert!(poll(wait.as_mut()).is_pending());
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    source.store(12, Ordering::SeqCst);
    assert!(poll(wait.as_mut()).is_pending());
    assert!(poll(stop.as_mut()).is_ready());
    drop(stop);
    assert!(poll(wait.as_mut()).is_pending());
    control.resume();
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 12 }));
}

// An admitted acquisition remains in flight until the consumer polls its next wait.
#[test]
fn admitted_acquisition_requires_next_wait() {
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(Arc::new(AtomicU64::new(8))));
    let mut wait = Box::pin(reader.wait());
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 8 }));
    drop(wait);
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    let mut next = Box::pin(reader.wait());
    assert!(poll(stop.as_mut()).is_pending());
    assert!(poll(next.as_mut()).is_pending());
    assert!(poll(stop.as_mut()).is_ready());
}

// Canceling the reader revokes its acknowledgement and requires a new polled wait.
#[test]
fn canceled_reader_revokes_acknowledgement() {
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(Arc::new(AtomicU64::new(0))));
    control.request_quiescence();
    let mut wait = Box::pin(reader.wait());
    assert!(poll(wait.as_mut()).is_pending());
    drop(wait);
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    let mut next = Box::pin(reader.wait());
    assert!(poll(next.as_mut()).is_pending());
    assert!(poll(stop.as_mut()).is_ready());
}

// Canceling the controller leaves admission blocked until an explicit resume.
#[test]
fn canceled_controller_keeps_request_active() {
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(Arc::new(AtomicU64::new(9))));
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    drop(stop);
    let mut wait = Box::pin(reader.wait());
    assert!(poll(wait.as_mut()).is_pending());
    control.resume();
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 9 }));
}

// A second request needs a fresh acknowledgement even if the reader has not run since resume.
#[test]
fn repeated_request_requires_fresh_acknowledgement() {
    let mut storage = Acquisition::<NoopRawMutex>::new();
    let (mut control, mut reader) = storage.split(Source(Arc::new(AtomicU64::new(9))));
    control.request_quiescence();
    let mut wait = Box::pin(reader.wait());
    assert!(poll(wait.as_mut()).is_pending());
    control.resume();
    let mut stop = Box::pin(control.quiesce());
    assert!(poll(stop.as_mut()).is_pending());
    assert!(poll(wait.as_mut()).is_pending());
    assert!(poll(stop.as_mut()).is_ready());
    drop(stop);
    control.resume();
    assert_eq!(poll(wait.as_mut()), Poll::Ready(Ready { at_us: 9 }));
}
