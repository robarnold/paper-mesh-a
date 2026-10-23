//! Encode display recovery records for aligned RTC word access.

const MAGIC: u32 = 0xd15a_7e00;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Stage {
    Beginning = 1,
    LogicReady,
    RailsRequested,
    RailsReady,
    Scanning,
    ShuttingDown,
    ControlsLow,
}

impl Stage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Beginning => "beginning",
            Self::LogicReady => "logic ready",
            Self::RailsRequested => "rails requested",
            Self::RailsReady => "rails ready",
            Self::Scanning => "scanning",
            Self::ShuttingDown => "shutting down",
            Self::ControlsLow => "controls low",
        }
    }

    fn from_word(word: u32) -> Option<Self> {
        match word & 0x0f {
            1 => Some(Self::Beginning),
            2 => Some(Self::LogicReady),
            3 => Some(Self::RailsRequested),
            4 => Some(Self::RailsReady),
            5 => Some(Self::Scanning),
            6 => Some(Self::ShuttingDown),
            7 => Some(Self::ControlsLow),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Status {
    Active = 0x10,
    Failed = 0x20,
    Interrupted = 0x30,
    Panicked = 0x40,
}

impl Status {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Panicked => "panic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub stage: Stage,
    pub status: Status,
}

impl Record {
    pub const fn beginning() -> Self {
        Self {
            stage: Stage::Beginning,
            status: Status::Active,
        }
    }

    pub const fn encode(self) -> u32 {
        MAGIC | self.stage as u32 | self.status as u32
    }

    pub fn decode(word: u32) -> Option<Self> {
        if word & 0xffff_ff00 != MAGIC {
            return None;
        }
        let stage = Stage::from_word(word)?;
        let status = match word & 0xf0 {
            0x10 => Status::Active,
            0x20 => Status::Failed,
            0x30 => Status::Interrupted,
            0x40 => Status::Panicked,
            _ => return None,
        };
        Some(Self { stage, status })
    }

    pub const fn after_boot(self) -> Self {
        // EN resets can report ChipPowerOn while RTC memory still contains a fault.
        // Valid retained state takes precedence over the reported reset reason.
        match self.status {
            Status::Active => Self {
                status: Status::Interrupted,
                ..self
            },
            _ => self,
        }
    }

    pub const fn progress(self, stage: Stage) -> Self {
        match self.status {
            Status::Active => Self { stage, ..self },
            _ => self,
        }
    }

    pub const fn failed(self) -> Self {
        match self.status {
            Status::Active => Self {
                status: Status::Failed,
                ..self
            },
            _ => self,
        }
    }

    pub const fn finish(self, shutdown_confirmed: bool, success: bool) -> Completion {
        Completion {
            retained: if shutdown_confirmed && success && matches!(self.status, Status::Active) {
                None
            } else {
                Some(self.failed())
            },
            release_watchdog: shutdown_confirmed,
        }
    }

    pub const fn panicked(self) -> Self {
        match self.status {
            Status::Active => Self {
                status: Status::Panicked,
                ..self
            },
            _ => self,
        }
    }
}

/// A separate panic record survives a simultaneous normal cleanup.
pub fn select(current: u32, panic: u32) -> Option<Record> {
    Record::decode(panic).or_else(|| Record::decode(current))
}

pub struct Completion {
    pub retained: Option<Record>,
    pub release_watchdog: bool,
}

// The deadline includes the refresh, driver release, and checked shutdown.
pub const fn timeout_ms() -> u64 {
    15_000
}

#[cfg(test)]
mod tests {
    use super::{Record, Stage, Status, select, timeout_ms};

    #[test]
    fn an_interrupted_operation_keeps_the_last_stage_across_resets() {
        for stage in [
            Stage::Beginning,
            Stage::LogicReady,
            Stage::RailsRequested,
            Stage::RailsReady,
            Stage::Scanning,
            Stage::ShuttingDown,
            Stage::ControlsLow,
        ] {
            let before = Record::beginning().progress(stage);
            let after = Record::decode(before.encode()).unwrap().after_boot();
            assert_eq!(after.stage, stage);
            assert_eq!(after.status, Status::Interrupted);
            assert_eq!(after.after_boot(), after);
        }
    }

    #[test]
    fn chip_power_on_is_not_evidence_that_rtc_memory_lost_power() {
        // Boot intentionally takes no reset-reason argument. A valid word survives
        // all reported reset reasons, including ChipPowerOn after an EN reset.
        let record = Record::beginning().progress(Stage::Scanning).after_boot();
        assert_eq!(
            Record::decode(record.encode()).unwrap().after_boot(),
            record
        );
    }

    #[test]
    fn cleared_or_invalid_rtc_memory_does_not_create_a_fault() {
        for word in [
            0,
            u32::MAX,
            0xd15a_7e00,
            0xd15a_7e18,
            0xd15a_7e51,
            0xd15b_7e11,
        ] {
            assert_eq!(Record::decode(word), None);
        }
    }

    #[test]
    fn panic_preserves_the_last_stage() {
        let record = Record::beginning().progress(Stage::Scanning).panicked();
        assert_eq!(record.status, Status::Panicked);
        assert_eq!(record.stage, Stage::Scanning);
        assert_eq!(record.after_boot(), record);
        assert_eq!(record.progress(Stage::ControlsLow), record);
        assert_eq!(record.failed(), record);
    }

    #[test]
    fn failure_cannot_be_erased_by_later_progress() {
        let record = Record::beginning().progress(Stage::RailsRequested).failed();
        assert_eq!(record.status, Status::Failed);
        assert_eq!(record.progress(Stage::ControlsLow), record);
        assert_eq!(record.after_boot(), record);
    }

    #[test]
    fn only_confirmed_shutdown_can_release_the_watchdog() {
        for success in [false, true] {
            let result = Record::beginning()
                .progress(Stage::ShuttingDown)
                .finish(false, success);
            assert!(!result.release_watchdog);
            assert_eq!(result.retained.unwrap().status, Status::Failed);
        }
    }

    #[test]
    fn a_scan_failure_stays_blocked_after_confirmed_shutdown() {
        let result = Record::beginning()
            .progress(Stage::ControlsLow)
            .finish(true, false);
        assert!(result.release_watchdog);
        assert_eq!(result.retained.unwrap().status, Status::Failed);
    }

    #[test]
    fn a_successful_shutdown_clears_the_refresh_record() {
        let result = Record::beginning()
            .progress(Stage::ControlsLow)
            .finish(true, true);
        assert!(result.release_watchdog);
        assert_eq!(result.retained, None);
    }

    #[test]
    fn a_panic_on_the_other_core_cannot_be_erased_by_successful_cleanup() {
        let record = Record::beginning().progress(Stage::Scanning).panicked();
        let result = record.finish(true, true);
        assert!(result.release_watchdog);
        assert_eq!(result.retained, Some(record));
    }

    #[test]
    fn normal_cleanup_cannot_erase_a_separate_panic_record() {
        let active = Record::beginning().progress(Stage::Scanning);
        let panic = active.panicked();
        assert_eq!(select(active.encode(), panic.encode()), Some(panic));
        assert_eq!(select(0, panic.encode()), Some(panic));
        assert_eq!(select(0, panic.encode()).unwrap().after_boot(), panic);
        assert_eq!(select(active.encode(), 0), Some(active));
        assert_eq!(select(0, 0), None);
    }

    #[test]
    fn display_deadline_keeps_the_normal_refresh_limit() {
        assert_eq!(timeout_ms(), 15_000);
    }
}
