//! A cancellation-safe handshake for a cooperative core stop.

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
        let _ = self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| match state {
                REQUESTED => Some(IDLE),
                PARKED => Some(RELEASING),
                _ => None,
            });
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
}
