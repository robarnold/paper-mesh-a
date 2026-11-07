//! A cancellation-safe handshake for a cooperative core stop.
//!
//! # Design choices
//!
//! - Allow cancellation of a park request so an abandoned suspend attempt cannot
//!   strand the other core.
//!
//! See [Manual light sleep](../../../docs/boards/lilygo-t5-s3-pro-power.md#manual-light-sleep).

use core::sync::atomic::{AtomicU32, Ordering};

const IDLE: u32 = 0;
const REQUESTED: u32 = 1;
const PARKED: u32 = 2;
const RELEASING: u32 = 3;

pub struct Park(AtomicU32);

impl Park {
    pub const fn new() -> Self {
        Self(AtomicU32::new(IDLE))
    }

    pub fn request(&self) -> bool {
        self.0
            .compare_exchange(IDLE, REQUESTED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn requested(&self) -> bool {
        self.0.load(Ordering::Acquire) == REQUESTED
    }

    pub fn claim(&self) -> bool {
        self.0
            .compare_exchange(REQUESTED, PARKED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn parked(&self) -> bool {
        self.0.load(Ordering::Acquire) == PARKED
    }

    pub fn release(&self) {
        let mut state = self.0.load(Ordering::Acquire);
        // Retry if the other core claims the request while cancellation is in progress.
        loop {
            let next = match state {
                REQUESTED => IDLE,
                PARKED => RELEASING,
                _ => return,
            };
            match self
                .0
                .compare_exchange_weak(state, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(observed) => state = observed,
            }
        }
    }

    pub fn finish(&self) {
        // The stopped core acknowledges release before another request can start.
        let _ = self
            .0
            .compare_exchange(RELEASING, IDLE, Ordering::AcqRel, Ordering::Acquire);
    }
}

#[cfg(test)]
mod tests {
    use super::Park;

    // Cancellation before the other core claims the request permits another request.
    #[test]
    fn cancellation_before_acknowledgement_does_not_stop_the_core() {
        let park = Park::new();
        assert!(park.request());
        assert!(park.requested());
        park.release();
        assert!(!park.claim());
        assert!(!park.parked());
        assert!(park.request());
    }

    // Each completed cycle permits another request only after release acknowledgment.
    #[test]
    fn repeated_cycles_wait_for_release_acknowledgement() {
        let park = Park::new();
        for _ in 0..100 {
            assert!(park.request());
            assert!(park.claim());
            assert!(park.parked());
            assert!(!park.request());
            park.release();
            assert!(!park.parked());
            assert!(!park.request());
            park.finish();
        }
    }

    // Concurrent cancellation and acknowledgment must leave the handshake ready for another request.
    #[test]
    fn cancellation_racing_with_acknowledgement_always_releases() {
        for _ in 0..100 {
            let park = Park::new();
            assert!(park.request());
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    if park.claim() {
                        while park.parked() {
                            std::thread::yield_now();
                        }
                        park.finish();
                    }
                });
                park.release();
            });
            assert!(park.request());
        }
    }

    // Repeated release preserves idle state and cannot bypass the stopped core's acknowledgment.
    #[test]
    fn repeated_release_preserves_acknowledgement_requirement() {
        let park = Park::new();
        park.release();
        park.release();
        assert!(park.request());
        assert!(park.claim());
        park.release();
        park.release();
        assert!(!park.request());
        park.finish();
        assert!(park.request());
    }
}
