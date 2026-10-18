//! Monotonic timers and observation age checks.

/// Periodic deadlines keep their original phase after a late poll.
pub struct PeriodicTimer {
    last_ms: u64,
    interval_ms: u64,
}

impl PeriodicTimer {
    /// The interval must be greater than zero.
    pub const fn new(started_at_ms: u64, interval_ms: u64) -> Self {
        assert!(interval_ms > 0);
        Self {
            last_ms: started_at_ms,
            interval_ms,
        }
    }

    pub fn next_deadline_ms(&self) -> u64 {
        self.last_ms.saturating_add(self.interval_ms)
    }

    /// A late poll consumes elapsed intervals and reports one event.
    pub fn poll(&mut self, now_ms: u64) -> bool {
        let elapsed = now_ms.saturating_sub(self.last_ms);
        if elapsed < self.interval_ms {
            return false;
        }
        self.last_ms += (elapsed / self.interval_ms) * self.interval_ms;
        true
    }
}

/// The caller selects the current time and maximum observation age.
#[derive(Clone, Copy)]
pub struct ObservationWindow {
    now_ms: u64,
    max_age_ms: u64,
}

impl ObservationWindow {
    pub const fn new(now_ms: u64, max_age_ms: u64) -> Self {
        Self { now_ms, max_age_ms }
    }

    /// A future timestamp has no valid age.
    pub fn age_ms(self, observed_at_ms: u64) -> Option<u64> {
        self.now_ms.checked_sub(observed_at_ms)
    }

    /// Observations expire at the maximum age.
    pub fn contains(self, observed_at_ms: u64) -> bool {
        self.age_ms(observed_at_ms)
            .is_some_and(|age| age < self.max_age_ms)
    }

    /// This method returns the earliest expiry after the current time.
    /// Timestamp addition saturates at the clock limit.
    pub fn next_expiry_ms(self, observations: impl IntoIterator<Item = u64>) -> Option<u64> {
        observations
            .into_iter()
            .map(|time| time.saturating_add(self.max_age_ms))
            .filter(|deadline| *deadline > self.now_ms)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_polls_keep_phase_without_repeated_events() {
        let mut timer = PeriodicTimer::new(100, 30);
        assert!(!timer.poll(99));
        assert!(!timer.poll(129));
        assert!(timer.poll(130));
        assert!(timer.poll(205));
        assert!(!timer.poll(205));
        assert_eq!(timer.next_deadline_ms(), 220);
    }

    #[test]
    fn timer_deadlines_saturate_without_overflow() {
        let mut timer = PeriodicTimer::new(u64::MAX - 25, 20);
        assert!(timer.poll(u64::MAX));
        assert_eq!(timer.next_deadline_ms(), u64::MAX);
        assert!(!timer.poll(u64::MAX));
    }

    #[test]
    #[should_panic]
    fn zero_interval_is_invalid() {
        PeriodicTimer::new(0, 0);
    }

    #[test]
    fn observation_age_rejects_future_and_expired_values() {
        let window = ObservationWindow::new(100, 50);
        assert_eq!(window.age_ms(101), None);
        assert!(!window.contains(101));
        assert!(window.contains(100));
        assert!(window.contains(51));
        assert!(!window.contains(50));
        assert!(!ObservationWindow::new(100, 0).contains(100));
        assert_eq!(window.next_expiry_ms([0, 50, 80, 90]), Some(130));
        assert_eq!(window.next_expiry_ms([0, 50]), None);
        assert_eq!(window.next_expiry_ms([]), None);
        assert_eq!(
            ObservationWindow::new(u64::MAX - 1, 50).next_expiry_ms([u64::MAX - 2]),
            Some(u64::MAX)
        );
    }
}
