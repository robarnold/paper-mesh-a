//! Exclusive ownership of the hardware recovery deadline.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Owner {
    Boot,
    Display,
    Suspend,
    PowerOff,
}

// The maximum suspend interval is one hour, with a 15-second recovery margin.
const MAX_TIMEOUT_MS: u64 = 3_615_000;

pub trait Watchdog {
    fn arm(&mut self, milliseconds: u64);
    fn disable(&mut self);
}

pub struct Manager<W> {
    watchdog: W,
    owner: Option<Owner>,
}

impl<W: Watchdog> Manager<W> {
    pub const fn new(watchdog: W) -> Self {
        Self {
            watchdog,
            owner: None,
        }
    }

    pub fn arm(&mut self, owner: Owner, milliseconds: u64) -> bool {
        if milliseconds == 0 || milliseconds > MAX_TIMEOUT_MS {
            return false;
        }
        match self.owner {
            Some(active) if active != owner => return false,
            // A second display operation must not extend the first deadline.
            Some(Owner::Display) => return false,
            _ => {}
        }
        self.owner = Some(owner);
        self.watchdog.arm(milliseconds);
        true
    }

    pub const fn idle(&self) -> bool {
        self.owner.is_none()
    }

    pub fn disable(&mut self, owner: Owner) -> bool {
        if self.owner != Some(owner) {
            return false;
        }
        self.watchdog.disable();
        self.owner = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Manager, Owner, Watchdog};

    #[derive(Debug, PartialEq)]
    enum Operation {
        Arm(u64),
        Disable,
    }

    #[derive(Default)]
    struct FakeWatchdog(Vec<Operation>);

    impl Watchdog for FakeWatchdog {
        fn arm(&mut self, milliseconds: u64) {
            self.0.push(Operation::Arm(milliseconds));
        }
        fn disable(&mut self) {
            self.0.push(Operation::Disable);
        }
    }

    #[test]
    fn other_owners_cannot_modify_an_active_deadline() {
        for active in [Owner::Boot, Owner::Display, Owner::Suspend, Owner::PowerOff] {
            for other in [Owner::Boot, Owner::Display, Owner::Suspend, Owner::PowerOff] {
                if other == active {
                    continue;
                }
                let mut manager = Manager::new(FakeWatchdog::default());
                assert!(manager.arm(active, 10_000));
                assert!(!manager.arm(other, 50_000));
                assert!(!manager.disable(other));
                assert_eq!(manager.watchdog.0, [Operation::Arm(10_000)]);
                assert_eq!(manager.owner, Some(active));
            }
        }
    }

    #[test]
    fn a_second_display_guard_cannot_extend_the_deadline() {
        let mut manager = Manager::new(FakeWatchdog::default());
        assert!(manager.arm(Owner::Display, 10_000));
        assert!(!manager.arm(Owner::Display, 35_000));
        assert_eq!(manager.watchdog.0, [Operation::Arm(10_000)]);
    }

    #[test]
    fn suspend_can_replace_its_own_sleep_deadline() {
        let mut manager = Manager::new(FakeWatchdog::default());
        assert!(manager.arm(Owner::Suspend, 15_000));
        assert!(manager.arm(Owner::Suspend, 3_615_000));
        assert!(manager.arm(Owner::Suspend, 15_000));
        assert!(manager.disable(Owner::Suspend));
        assert_eq!(
            manager.watchdog.0,
            [
                Operation::Arm(15_000),
                Operation::Arm(3_615_000),
                Operation::Arm(15_000),
                Operation::Disable,
            ]
        );
    }

    #[test]
    fn invalid_timeouts_leave_the_existing_hardware_deadline_unchanged() {
        let mut manager = Manager::new(FakeWatchdog::default());
        assert!(manager.arm(Owner::Suspend, 15_000));
        assert!(!manager.arm(Owner::Suspend, 0));
        assert!(!manager.arm(Owner::Suspend, 3_615_001));
        assert!(!manager.arm(Owner::Suspend, u64::MAX));
        assert_eq!(manager.watchdog.0, [Operation::Arm(15_000)]);
    }

    #[test]
    fn confirmed_completion_allows_another_owner() {
        let mut manager = Manager::new(FakeWatchdog::default());
        assert!(manager.arm(Owner::Display, 10_000));
        assert!(manager.disable(Owner::Display));
        assert!(!manager.disable(Owner::Display));
        assert!(manager.arm(Owner::PowerOff, 15_000));
        assert_eq!(
            manager.watchdog.0,
            [
                Operation::Arm(10_000),
                Operation::Disable,
                Operation::Arm(15_000),
            ]
        );
    }

    #[test]
    fn boot_recovery_blocks_display_until_shutdown_is_confirmed() {
        let mut manager = Manager::new(FakeWatchdog::default());
        assert!(manager.arm(Owner::Boot, 15_000));
        assert!(!manager.arm(Owner::Display, 10_000));
        assert!(!manager.disable(Owner::PowerOff));
        assert!(manager.disable(Owner::Boot));
        assert!(manager.arm(Owner::Display, 10_000));
    }

    #[test]
    fn a_new_boot_has_no_stale_lease() {
        let mut previous = Manager::new(FakeWatchdog::default());
        assert!(previous.arm(Owner::Display, 10_000));
        let mut next_boot = Manager::new(FakeWatchdog::default());
        assert!(next_boot.arm(Owner::PowerOff, 15_000));
    }
}
