//! Suspend requests and charge comparisons within one application epoch.

use crate::{
    input::buttons::ButtonId,
    power::{GaugeState, PowerAvailability, PowerSnapshot},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuspendRequest {
    pub duration_ms: u64,
    pub touch_wake: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WakeReason {
    Timer,
    Control(ButtonId),
    /// A touch-controller interrupt does not establish a deliberate tap.
    TouchInterrupt,
    /// A shared input interrupt could not be attributed to a readable control.
    UnidentifiedInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuspendError {
    Busy,
    Preparation,
    WakeActive,
    WakeLocked,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuspendReport {
    pub elapsed_ms: u64,
    /// Observed wake events. Array order does not establish event order.
    pub wake_reasons: [Option<WakeReason>; 4],
    pub before: PowerSnapshot,
    pub after: PowerSnapshot,
}

impl SuspendReport {
    /// Positive values mean net charge loss. Negative values mean net charge gain.
    /// Gauge estimates can change with temperature, load, and profile corrections.
    pub fn charge_loss_mah(&self) -> Option<i32> {
        if self.after.battery.observation?.observed_at_ms
            <= self.before.battery.observation?.observed_at_ms
        {
            return None;
        }
        let capacity = |snapshot: PowerSnapshot| {
            if snapshot.battery.availability != PowerAvailability::Ready {
                return None;
            }
            let battery = snapshot.battery.observation?.value;
            (battery.present && battery.gauge == GaugeState::Ready)
                .then_some(battery.remaining_capacity_mah)
                .flatten()
        };
        Some(i32::from(capacity(self.before)?) - i32::from(capacity(self.after)?))
    }

    /// Endpoint checks cannot exclude external power changes during suspend.
    pub fn external_power_at_endpoints(&self) -> Option<bool> {
        if self.after.charger.observation?.observed_at_ms
            <= self.before.charger.observation?.observed_at_ms
        {
            return None;
        }
        let supplied = |snapshot: PowerSnapshot| {
            (snapshot.charger.availability == PowerAvailability::Ready)
                .then_some(snapshot.charger.observation)
                .flatten()
                .map(|observation| observation.value.power_good)
        };
        Some(supplied(self.before)? || supplied(self.after)?)
    }
}
