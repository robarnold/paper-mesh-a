//! Deterministic location observations. The simulator does not model satellite reception.

use paper_mesh_app::platform::location::{
    Coordinates, DISPLAY_SATELLITES, Location, LocationAvailability, LocationObservation,
    LocationSnapshot, MotionObservation, ReceiverActivity, SatelliteObservation, SatelliteSignal,
    SatelliteSystem, UtcObservation, UtcTime,
};
use serde::{Deserialize, Serialize};

pub const ACQUISITION_MS: u64 = 3_000;
const REPORT_MS: u64 = 1_000;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationScenario {
    #[default]
    Fix,
    Acquiring,
    NoSignal,
    MessagesOnly,
    StaleSatellites,
    LostFix,
    Silent,
    Unavailable,
    Fault,
    ZeroCoordinates,
    BelowSeaLevel,
    MissingElevation,
    Stopped,
    MissingMotion,
    MissingUtc,
    StaleMotion,
    StaleUtc,
}

#[derive(Default)]
pub struct SimLocation {
    pub scenario: LocationScenario,
    started_at_ms: u64,
    messages_before: u32,
    prior_activity: Option<ReceiverActivity>,
}

impl SimLocation {
    pub fn set_scenario(&mut self, scenario: LocationScenario, now_ms: u64) {
        self.prior_activity = self.snapshot(now_ms).activity;
        self.messages_before = self
            .prior_activity
            .map_or(0, |activity| activity.valid_messages);
        self.scenario = scenario;
        self.started_at_ms = now_ms;
    }

    pub fn next_deadline_ms(&self, now_ms: u64) -> Option<u64> {
        match self.scenario {
            LocationScenario::Silent | LocationScenario::Unavailable | LocationScenario::Fault => {
                None
            }
            _ => Some(
                self.started_at_ms.saturating_add(
                    (now_ms.saturating_sub(self.started_at_ms) / REPORT_MS + 1)
                        .saturating_mul(REPORT_MS),
                ),
            ),
        }
    }
}

impl Location for SimLocation {
    fn snapshot(&mut self, now_ms: u64) -> LocationSnapshot {
        let elapsed = now_ms.saturating_sub(self.started_at_ms);
        let availability = match self.scenario {
            LocationScenario::Unavailable => LocationAvailability::Unavailable,
            LocationScenario::Fault => LocationAvailability::Fault,
            _ => LocationAvailability::Ready,
        };
        let has_fix = match self.scenario {
            LocationScenario::Fix => elapsed >= ACQUISITION_MS,
            LocationScenario::LostFix => elapsed < ACQUISITION_MS,
            LocationScenario::Silent
            | LocationScenario::ZeroCoordinates
            | LocationScenario::BelowSeaLevel
            | LocationScenario::MissingElevation
            | LocationScenario::Stopped
            | LocationScenario::MissingMotion
            | LocationScenario::MissingUtc
            | LocationScenario::StaleMotion
            | LocationScenario::StaleUtc => true,
            _ => false,
        };
        let coordinates = if self.scenario == LocationScenario::ZeroCoordinates {
            Coordinates::new(0, 0)
        } else {
            Coordinates::new(37_774_900, -122_419_400)
        };
        let received_at_ms = self.started_at_ms
            + if self.scenario == LocationScenario::Silent {
                0
            } else {
                elapsed / REPORT_MS * REPORT_MS
            };
        let ready = availability == LocationAvailability::Ready;
        let utc_received_at_ms = if self.scenario == LocationScenario::StaleUtc {
            self.started_at_ms
        } else {
            received_at_ms
        };
        let utc_seconds = ((utc_received_at_ms - self.started_at_ms) / 1_000
            + if self.scenario == LocationScenario::ZeroCoordinates {
                0
            } else {
                7 * 3_600 + 15 * 60 + 56
            })
            % 86_400;
        let mut strongest = [None; DISPLAY_SATELLITES];
        for (index, level) in [44, 40, 37, 33, 26, 18, 0, 0].into_iter().enumerate() {
            strongest[index] = Some(SatelliteSignal {
                system: if index < 4 {
                    SatelliteSystem::Gps
                } else {
                    SatelliteSystem::BeiDou
                },
                id: (index + 1) as u16,
                cn0_db_hz: if self.scenario == LocationScenario::NoSignal || level == 0 {
                    None
                } else if has_fix {
                    Some(level)
                } else {
                    Some(level / 2)
                },
            });
        }
        LocationSnapshot {
            availability,
            observation: (ready && self.scenario != LocationScenario::MessagesOnly).then_some(
                LocationObservation {
                    observed_at_ms: received_at_ms,
                    coordinates: coordinates.filter(|_| has_fix),
                    elevation_mm: if has_fix {
                        match self.scenario {
                            LocationScenario::ZeroCoordinates => Some(0),
                            LocationScenario::BelowSeaLevel => Some(-430_500),
                            LocationScenario::MissingElevation => None,
                            _ => Some(75_700),
                        }
                    } else {
                        None
                    },
                    satellites_used: Some(if has_fix { 8 } else { 0 }),
                },
            ),
            motion: (ready && self.scenario != LocationScenario::MessagesOnly).then_some(
                MotionObservation {
                    observed_at_ms: if self.scenario == LocationScenario::StaleMotion {
                        self.started_at_ms
                    } else {
                        received_at_ms
                    },
                    speed_mm_s: (has_fix && self.scenario != LocationScenario::MissingMotion)
                        .then_some(if self.scenario == LocationScenario::Stopped {
                            0
                        } else {
                            2_780
                        }),
                    course_millidegrees: (has_fix
                        && self.scenario != LocationScenario::MissingMotion)
                        .then_some(315_000),
                },
            ),
            utc: (ready && has_fix && self.scenario != LocationScenario::MissingUtc).then_some(
                UtcObservation {
                    observed_at_ms: utc_received_at_ms,
                    time: UtcTime::new(
                        (utc_seconds / 3_600) as u8,
                        (utc_seconds / 60 % 60) as u8,
                        (utc_seconds % 60) as u8,
                    )
                    .unwrap(),
                },
            ),
            activity: if ready {
                Some(ReceiverActivity {
                    received_at_ms,
                    valid_messages: self.messages_before.saturating_add(
                        ((received_at_ms - self.started_at_ms) / REPORT_MS + 1)
                            .saturating_mul(10)
                            .min(u64::from(u32::MAX)) as u32,
                    ),
                })
            } else {
                self.prior_activity
            },
            satellites: (ready && self.scenario != LocationScenario::MessagesOnly).then_some(
                SatelliteObservation {
                    observed_at_ms: if self.scenario == LocationScenario::StaleSatellites {
                        self.started_at_ms
                    } else {
                        received_at_ms
                    },
                    satellites_in_view: 8,
                    satellites_with_signal: if self.scenario == LocationScenario::NoSignal {
                        0
                    } else {
                        6
                    },
                    strongest,
                    truncated: false,
                },
            ),
        }
    }
}
