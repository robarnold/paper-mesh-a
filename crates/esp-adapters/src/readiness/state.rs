//! Private GPIO capture state, independent of acquisition delivery.
//!
//! # Design choices
//!
//! - Retain one coalesced notification even when no reader is waiting.
//! - Mask an active level after capture to avoid repeated interrupt entry until that notification is consumed.
//! - Keep wake capture independent of both the hardware latch and readiness consumption.
//!
//! See [GPIO readiness provider](../../../../docs/adapters/gpio-readiness.md).

use super::{ArmError, RestorationObservation, WakeCapture};
use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};
use paper_mesh_platform::readiness::Ready;

// Private implementation seam for deterministic tests, not an adapter GPIO contract.
pub(super) trait Input {
    /// Sample whether the pin has its configured active level.
    fn active(&self) -> bool;
    /// Inspect this pin's interrupt latch without acknowledging it.
    fn pending(&self) -> bool;
    /// Disable interrupt delivery without clearing its latch.
    fn mask(&mut self);
    /// Acknowledge this pin's interrupt latch without changing its physical level.
    fn clear(&mut self);
    /// Enable level-sensitive delivery, including activation during the enable operation.
    fn enable(&mut self);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Stopped,
    Capture,
    Wake,
}
struct Inner<P> {
    pin: P,
    mode: Mode,
    waiting: bool,
    pending: Option<Ready>,
    wake_captured: bool,
}
impl<P: Input> Inner<P> {
    /// Mask and acknowledge the pin while retaining the earliest unconsumed observation.
    fn capture(&mut self, at_us: u64) {
        self.pin.mask();
        self.pin.clear();
        self.pending.get_or_insert(Ready { at_us });
    }
    /// Keep retained readiness masked, capture an active condition, or enable future level delivery.
    fn rearm_capture(&mut self, at_us: u64) {
        if self.pending.is_some() {
            self.pin.mask();
        } else if self.pin.pending() || self.pin.active() {
            self.capture(at_us);
        } else {
            // Level-sensitive delivery catches activation during or after this enable.
            self.pin.enable();
        }
    }
}
/// Serialize pin access, retained notifications, and ownership of one ordinary reader.
/// Mode changes do not establish acquisition quiescence. The caller must obtain it separately before wake arming.
pub(super) struct State<M: RawMutex, P: Input> {
    inner: Mutex<M, RefCell<Inner<P>>>,
    changed: Signal<M, ()>,
}
impl<M: RawMutex, P: Input> State<M, P> {
    /// Take pin ownership, mask delivery, and clear its existing hardware latch.
    /// Capture remains stopped until `start_capture`. No reader or wake interval exists initially.
    pub(super) fn new(mut pin: P) -> Self {
        pin.mask();
        pin.clear();
        Self {
            inner: Mutex::new(RefCell::new(Inner {
                pin,
                mode: Mode::Stopped,
                waiting: false,
                pending: None,
                wake_captured: false,
            })),
            changed: Signal::new(),
        }
    }
    /// Start capture once, after interrupt registration. Observe an active level immediately.
    /// Panic if capture has already started. Timestamps use runtime monotonic microseconds.
    pub(super) fn start_capture(&self, at_us: u64) {
        self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            assert_eq!(inner.mode, Mode::Stopped, "GPIO capture already started");
            inner.mode = Mode::Capture;
            inner.rearm_capture(at_us);
        });
    }
    /// Deliver retained readiness or wait for capture. The callback supplies monotonic microseconds.
    /// Panic unless ordinary capture is active and no other reader owns delivery.
    /// Completion rearms capture. Cancellation releases delivery without discarding pending readiness.
    pub(super) async fn wait_for_readiness(&self, now: fn() -> u64) -> Ready {
        self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            assert_eq!(
                inner.mode,
                Mode::Capture,
                "ordinary delivery is unavailable"
            );
            assert!(!inner.waiting, "only one readiness reader is permitted");
            inner.waiting = true;
        });
        let _guard = DeliveryWait(self);
        loop {
            let ready = self.inner.lock(|inner| {
                let mut inner = inner.borrow_mut();
                inner.rearm_capture(now());
                let ready = inner.pending.take();
                if ready.is_some() {
                    // Resume capture independently of whether the consumer starts another wait.
                    inner.rearm_capture(now());
                }
                ready
            });
            if let Some(ready) = ready {
                return ready;
            }
            self.changed.wait().await;
        }
    }
    /// Release reader ownership without changing capture state or pending readiness.
    fn did_end_delivery(&self) {
        self.inner.lock(|inner| inner.borrow_mut().waiting = false);
        // Capture and its pending event deliberately survive cancellation of reader delivery.
    }
    /// Process this pin's pending latch and retain readiness before notifying the reader.
    /// Return true only for the first capture in the current wake interval, to dispatch the wake callback.
    /// Ignore unrelated interrupts. While stopped, clear this pin's latch without publishing readiness.
    pub(super) fn did_interrupt(&self, at_us: u64) -> bool {
        let (notify, wake) = self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            if !inner.pin.pending() {
                return (false, false);
            }
            if inner.mode == Mode::Stopped {
                inner.pin.mask();
                inner.pin.clear();
                return (false, false);
            }
            inner.capture(at_us);
            let wake = inner.mode == Mode::Wake && !inner.wake_captured;
            if inner.mode == Mode::Wake {
                inner.wake_captured = true;
            }
            (true, wake)
        });
        if notify {
            self.changed.signal(());
        }
        wake
    }
    /// Begin a fresh wake interval after acquisition quiescence. Preserve ordinary pending readiness.
    /// Refuse without mutation unless capture is active, no wait owns delivery, and the pin is inactive.
    pub(super) fn arm_wake_capture(&self, at_us: u64) -> Result<(), ArmError> {
        self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            if inner.mode != Mode::Capture {
                return Err(ArmError::NotCapturing);
            }
            if inner.waiting {
                return Err(ArmError::ReaderActive);
            }
            if inner.pin.active() {
                return Err(ArmError::InputActive);
            }
            // Preserve readiness captured before wake admission even if its handler has not run.
            if inner.pin.pending() {
                inner.capture(at_us);
            }
            inner.wake_captured = false;
            inner.mode = Mode::Wake;
            inner.pin.enable();
            Ok(())
        })
    }
    /// End an armed interval and resume ordinary capture, including any unhandled hardware latch.
    /// Return None if no interval is armed. Otherwise return its capture result, retaining restoration evidence.
    pub(super) fn finish_wake_capture(&self, at_us: u64) -> Option<WakeCapture> {
        self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            if inner.mode != Mode::Wake {
                return None;
            }
            if inner.pin.pending() {
                inner.capture(at_us);
                inner.wake_captured = true;
            }
            inner.pin.mask();
            inner.pin.clear();
            inner.mode = Mode::Capture;
            inner.rearm_capture(at_us);
            Some(if inner.wake_captured {
                WakeCapture::Captured
            } else {
                WakeCapture::NotCaptured
            })
        })
    }
    /// Consume retained wake evidence and sample the current level after wake completion.
    /// Ordinary readiness remains pending. Repeated calls resample the level but do not repeat consumed wake evidence.
    pub(super) fn take_restoration_observation(&self) -> RestorationObservation {
        self.inner.lock(|inner| {
            let mut inner = inner.borrow_mut();
            let observed = RestorationObservation {
                wake: if inner.wake_captured {
                    WakeCapture::Captured
                } else {
                    WakeCapture::NotCaptured
                },
                input_active: inner.pin.active(),
            };
            inner.wake_captured = false;
            observed
        })
    }
}
struct DeliveryWait<'a, M: RawMutex, P: Input>(&'a State<M, P>);
impl<M: RawMutex, P: Input> Drop for DeliveryWait<'_, M, P> {
    fn drop(&mut self) {
        self.0.did_end_delivery();
    }
}

#[cfg(test)]
mod tests;
