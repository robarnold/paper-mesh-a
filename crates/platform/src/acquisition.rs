//! Single-reader acquisition admission and quiescence.
//!
//! # Design choices
//!
//! - Treat polling the next wait as proof that the preceding acquisition released its resources and published its result.
//! - Reuse pause coordination internally while keeping the acquisition loop free of a separate pause branch.
//! - Retain admitted readiness across suspend races instead of equating notification with a device report.
//!
//! See [acquisition control](../../../docs/platform/readiness.md#acquisition-control).

use crate::{
    readiness::{Readiness, Ready},
    worker::{Pause, PauseControl, PauseWorker},
};
use core::{
    future::{Future, poll_fn},
    pin::pin,
    task::Poll,
};
use embassy_sync::blocking_mutex::raw::RawMutex;

/// Storage for exactly one acquisition reader and its controller.
pub struct Acquisition<M: RawMutex> {
    pause: Pause<M>,
}
impl<M: RawMutex> Default for Acquisition<M> {
    fn default() -> Self {
        Self::new()
    }
}
impl<M: RawMutex> Acquisition<M> {
    pub const fn new() -> Self {
        Self {
            pause: Pause::new(),
        }
    }
    pub fn split<R: Readiness>(&mut self, source: R) -> (Control<'_, M>, Reader<'_, M, R>) {
        let (control, reader) = self.pause.split();
        (
            Control { pause: control },
            Reader {
                pause: reader,
                source,
                pending: None,
            },
        )
    }
}

/// Controller endpoint that blocks acquisition admission, awaits quiescence, and releases the reader after restoration.
pub struct Control<'a, M: RawMutex> {
    pause: PauseControl<'a, M>,
}
impl<M: RawMutex> Control<'_, M> {
    /// Block new admissions without claiming that an acquisition already in flight has finished.
    pub fn request_quiescence(&mut self) {
        self.pause.request_pause();
    }
    /// Wait for the reader's next polled wait. Cancellation leaves the request active until resume.
    pub async fn quiesce(&mut self) {
        self.pause.pause().await;
    }
    /// Release delivery after the consumer has restored its logical state.
    pub fn resume(&mut self) {
        self.pause.resume();
    }
}

/// Single consumer endpoint that admits acquisitions and acknowledges quiescence between completed acquisitions.
pub struct Reader<'a, M: RawMutex, R: Readiness> {
    pause: PauseWorker<'a, M>,
    source: R,
    pending: Option<Ready>,
}
impl<M: RawMutex, R: Readiness> Reader<'_, M, R> {
    /// Poll only after releasing all acquisition resources and publishing the previous result.
    /// Constructing an unpolled future establishes no idle acknowledgement.
    /// Dropping a pending wait revokes any acknowledgement. Poll a new wait before acquiring again.
    pub async fn wait(&mut self) -> Ready {
        loop {
            self.pause.checkpoint().await;
            if let Some(ready) = self.pending.take() {
                return ready;
            }
            let result = {
                let mut pause_requested = pin!(self.pause.requested());
                let mut ready = pin!(self.source.wait());
                poll_fn(|cx| {
                    // Prioritize pause so simultaneous readiness cannot admit new work before quiescence.
                    // The source retains readiness for delivery after resume.
                    if pause_requested.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(None);
                    }
                    ready.as_mut().poll(cx).map(Some)
                })
                .await
            };
            // A pause can arrive after the request future was polled but before readiness completed.
            // Retain that event while the next checkpoint confirms whether admission is allowed.
            if let Some(ready) = result {
                self.pending = Some(ready);
            }
        }
    }
}

#[cfg(test)]
mod tests;
