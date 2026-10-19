//! Deterministic power observations. These values are not hardware measurements.

use paper_mesh_platform::power::{
    BatteryState, ChargeFault, ChargeState, ChargerFaults, ChargerState, GaugeState, Power,
    PowerAvailability, PowerReading, PowerSnapshot, PowerSource, ThermistorState,
};
use serde::{Deserialize, Serialize};

const REPORT_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerScenario {
    #[default]
    Battery,
    Charging,
    Full,
    LowBattery,
    NoBattery,
    Initializing,
    GaugeFault,
    ChargerFault,
    TemperatureFault,
    Unavailable,
    Stale,
}

#[derive(Default)]
pub struct SimPower {
    pub scenario: PowerScenario,
    started_at_ms: u64,
}

impl SimPower {
    pub fn set_scenario(&mut self, scenario: PowerScenario, now_ms: u64) {
        self.scenario = scenario;
        self.started_at_ms = now_ms;
    }

    pub fn next_deadline_ms(&self, now_ms: u64) -> Option<u64> {
        match self.scenario {
            PowerScenario::Stale | PowerScenario::Unavailable => None,
            _ => Some(
                self.started_at_ms.saturating_add(
                    (now_ms.saturating_sub(self.started_at_ms) / REPORT_MS + 1)
                        .saturating_mul(REPORT_MS),
                ),
            ),
        }
    }
}

impl Power for SimPower {
    fn snapshot(&mut self, now_ms: u64) -> PowerSnapshot {
        let scenario = self.scenario;
        if scenario == PowerScenario::Unavailable {
            return PowerSnapshot {
                battery: PowerReading::unavailable(PowerAvailability::Unavailable),
                charger: PowerReading::unavailable(PowerAvailability::Unavailable),
            };
        }
        let at_ms = if scenario == PowerScenario::Stale {
            self.started_at_ms
        } else {
            self.started_at_ms + now_ms.saturating_sub(self.started_at_ms) / REPORT_MS * REPORT_MS
        };
        let charging = scenario == PowerScenario::Charging;
        let full = scenario == PowerScenario::Full;
        let low = scenario == PowerScenario::LowBattery;
        let absent = scenario == PowerScenario::NoBattery;
        let hot = scenario == PowerScenario::TemperatureFault;
        let mut battery = BatteryState {
            present: true,
            gauge: GaugeState::Ready,
            state_of_charge_percent: Some(if full {
                100
            } else if low {
                4
            } else {
                68
            }),
            state_of_health_percent: Some(94),
            voltage_mv: Some(if full {
                4192
            } else if low {
                3290
            } else {
                3890
            }),
            current_ma: Some(if charging {
                640
            } else if full || hot {
                0
            } else {
                -180
            }),
            average_power_mw: Some(if charging {
                2490
            } else if full || hot {
                0
            } else {
                -700
            }),
            gauge_temperature_centi_c: Some(if hot { 5500 } else { 2575 }),
            remaining_capacity_mah: Some(if full {
                2820
            } else if low {
                113
            } else {
                1918
            }),
            full_capacity_mah: Some(2820),
            design_capacity_mah: Some(3000),
            cycles: Some(42),
            average_current_ma: Some(if charging {
                620
            } else if full || hot {
                0
            } else {
                -175
            }),
            time_to_empty_minutes: if charging || full || hot {
                None
            } else {
                Some(if low { 37 } else { 639 })
            },
            time_to_full_minutes: charging.then_some(95),
            fully_charged: full,
            discharge_active: !charging && !full && !hot,
            charge_inhibited: hot,
            overtemperature: hot,
            shutdown_requested: low,
            ..BatteryState::default()
        };
        if absent || scenario == PowerScenario::Initializing {
            battery = BatteryState {
                present: !absent,
                ..BatteryState::default()
            };
        }
        let faults = if hot {
            ChargerFaults {
                charge: ChargeFault::ThermalShutdown,
                thermistor: ThermistorState::Hot,
                ..ChargerFaults::default()
            }
        } else {
            ChargerFaults::default()
        };
        let charger = ChargerState {
            source: if charging || full || absent || hot {
                PowerSource::Usb
            } else {
                PowerSource::None
            },
            charge: if charging {
                ChargeState::Fast
            } else if full {
                ChargeState::Complete
            } else {
                ChargeState::Idle
            },
            power_good: charging || full || absent || hot,
            charge_enabled: true,
            input_suspended: false,
            input_limit_ma: 1500,
            charge_limit_ma: 1024,
            charge_voltage_mv: 4208,
            termination_current_ma: 128,
            input_voltage_limit_mv: 4400,
            input_current_limited: false,
            input_voltage_limited: false,
            thermal_regulation: hot,
            minimum_system_voltage: low,
            faults,
            latched_faults: faults,
        };
        PowerSnapshot {
            battery: if scenario == PowerScenario::GaugeFault {
                PowerReading::unavailable(PowerAvailability::Fault)
            } else {
                PowerReading::ready(battery, at_ms)
            },
            charger: if scenario == PowerScenario::ChargerFault {
                PowerReading::unavailable(PowerAvailability::Fault)
            } else {
                PowerReading::ready(charger, at_ms)
            },
        }
    }
}
