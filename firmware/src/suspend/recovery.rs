//! Reserve the shared RTC watchdog for a suspend experiment.

use crate::recovery::{self, Owner};
use esp_hal::rtc_cntl::Rwdt;

pub struct Recovery;

impl Recovery {
    pub fn new(watchdog: Rwdt) -> Self {
        recovery::initialize(watchdog);
        Self
    }

    pub fn arm(&mut self, milliseconds: u64) -> bool {
        recovery::arm(Owner::Suspend, milliseconds)
    }

    pub fn disable(&mut self) -> bool {
        recovery::disable(Owner::Suspend)
    }
}
