//! Cooperative pause coordination between one controller and one worker.
//!
//! # Design choices
//!
//! - Separate request and acknowledgement so the controller cannot infer resource release from a request alone.
//! - Keep notifications separate from state so coalesced or stale signals cannot acknowledge another pause.
//! - Let the backend supply locking without selecting an executor, timer, or power policy.
//!
//! See [Worker coordination](../../../docs/platform/worker-coordination.md).

use core::cell::Cell;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Running,
    Requested,
    Paused,
}

/// Storage for one cooperative pause channel. The runtime owns its lifetime.
pub struct Pause<M: RawMutex> {
    state: Mutex<M, Cell<State>>,
    worker_changed: Signal<M, ()>,
    controller_changed: Signal<M, ()>,
}

impl<M: RawMutex> Default for Pause<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> Pause<M> {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(Cell::new(State::Running)),
            worker_changed: Signal::new(),
            controller_changed: Signal::new(),
        }
    }

    /// Borrow unique endpoints. Neither endpoint can be cloned or split again.
    pub fn split(&mut self) -> (PauseControl<'_, M>, PauseWorker<'_, M>) {
        (PauseControl { shared: self }, PauseWorker { shared: self })
    }
}

/// Request, await, and release a worker pause. Dropping a wait does not release the request.
pub struct PauseControl<'a, M: RawMutex> {
    shared: &'a Pause<M>,
}

impl<M: RawMutex> PauseControl<'_, M> {
    /// Request a pause without waiting. This does not establish resource release.
    pub fn request_pause(&mut self) {
        self.shared.state.lock(|state| {
            if state.get() == State::Running {
                state.set(State::Requested);
            }
        });
        self.shared.worker_changed.signal(());
    }

    /// Wait until the worker acknowledges a safe checkpoint.
    /// Cancellation leaves the request active until an explicit resume or system reset.
    pub async fn pause(&mut self) {
        self.request_pause();
        while !self.shared.state.lock(|state| state.get() == State::Paused) {
            self.shared.controller_changed.wait().await;
        }
    }

    /// Release a complete or partial pause. A later pause requires a new acknowledgement.
    pub fn resume(&mut self) {
        self.shared.state.lock(|state| state.set(State::Running));
        self.shared.worker_changed.signal(());
    }
}

/// Worker endpoint. Reach a checkpoint after releasing resources and publishing completed work.
pub struct PauseWorker<'a, M: RawMutex> {
    shared: &'a Pause<M>,
}

impl<M: RawMutex> PauseWorker<'_, M> {
    /// Wait for a pause request. Callers can race this future against their normal work deadline.
    /// Dropping this wait does not consume the request or acknowledge a pause.
    pub async fn requested(&mut self) {
        while self
            .shared
            .state
            .lock(|state| state.get() == State::Running)
        {
            self.shared.worker_changed.wait().await;
        }
    }

    /// Acknowledge a request and wait for release, or return immediately without a request.
    /// If canceled, call this method again before resuming work. Cancellation revokes the acknowledgement.
    pub async fn checkpoint(&mut self) {
        let _acknowledgement = Acknowledgement {
            shared: self.shared,
        };
        loop {
            let paused = self.shared.state.lock(|state| {
                if state.get() == State::Running {
                    return false;
                }
                state.set(State::Paused);
                true
            });
            if !paused {
                return;
            }
            self.shared.controller_changed.signal(());
            self.shared.worker_changed.wait().await;
        }
    }
}

struct Acknowledgement<'a, M: RawMutex> {
    shared: &'a Pause<M>,
}

impl<M: RawMutex> Drop for Acknowledgement<'_, M> {
    fn drop(&mut self) {
        self.shared.state.lock(|state| {
            if state.get() == State::Paused {
                state.set(State::Requested);
            }
        });
        self.shared.controller_changed.signal(());
    }
}

#[cfg(test)]
mod tests;
