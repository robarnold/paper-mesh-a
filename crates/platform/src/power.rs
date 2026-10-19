//! Battery measurements and charger observations. This interface has no control operations.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAvailability {
    Starting,
    Ready,
    Unavailable,
    Fault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerObservation<T> {
    pub observed_at_ms: u64,
    pub value: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerReading<T> {
    pub availability: PowerAvailability,
    pub observation: Option<PowerObservation<T>>,
}

impl<T> PowerReading<T> {
    pub const fn unavailable(availability: PowerAvailability) -> Self {
        Self {
            availability,
            observation: None,
        }
    }

    pub const fn ready(value: T, observed_at_ms: u64) -> Self {
        Self {
            availability: PowerAvailability::Ready,
            observation: Some(PowerObservation {
                observed_at_ms,
                value,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GaugeState {
    #[default]
    Initializing,
    Ready,
    Configuring,
    Calibrating,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatteryState {
    pub present: bool,
    pub gauge: GaugeState,
    /// True only when the backend has verified the cell profile and calibration.
    pub estimates_verified: bool,
    pub state_of_charge_percent: Option<u8>,
    pub state_of_health_percent: Option<u8>,
    pub voltage_mv: Option<u16>,
    /// Positive current enters the battery. Negative current leaves the battery.
    pub current_ma: Option<i16>,
    /// Average current uses the same sign convention as instantaneous current.
    pub average_current_ma: Option<i16>,
    /// Positive power enters the battery. Negative power leaves the battery.
    pub average_power_mw: Option<i16>,
    /// Temperature of the gauge silicon, in hundredths of a degree Celsius.
    pub gauge_temperature_centi_c: Option<i32>,
    pub remaining_capacity_mah: Option<u16>,
    pub full_capacity_mah: Option<u16>,
    pub design_capacity_mah: Option<u16>,
    pub cycles: Option<u16>,
    pub time_to_empty_minutes: Option<u16>,
    pub time_to_full_minutes: Option<u16>,
    pub fully_charged: bool,
    pub fully_discharged: bool,
    pub discharge_active: bool,
    pub charge_inhibited: bool,
    pub overtemperature: bool,
    pub shutdown_requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Charger input classification, which need not identify the physical power connection.
pub enum PowerSource {
    None,
    Usb,
    Adapter,
    Output,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeState {
    Idle,
    Precharge,
    Fast,
    Complete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChargeFault {
    #[default]
    None,
    Input,
    ThermalShutdown,
    SafetyTimer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThermistorState {
    #[default]
    Normal,
    Warm,
    Cool,
    Cold,
    Hot,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChargerFaults {
    pub watchdog_expired: bool,
    pub boost: bool,
    pub battery_overvoltage: bool,
    pub charge: ChargeFault,
    pub thermistor: ThermistorState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChargerState {
    /// USB and adapter classes do not establish a physical connection or USB data session.
    pub source: PowerSource,
    pub charge: ChargeState,
    pub power_good: bool,
    pub charge_enabled: bool,
    pub input_suspended: bool,
    /// Register limits are settings, not measured currents or effective electrical limits.
    pub input_limit_ma: u16,
    pub charge_limit_ma: u16,
    pub charge_voltage_mv: u16,
    pub termination_current_ma: u16,
    pub input_voltage_limit_mv: u16,
    pub input_current_limited: bool,
    pub input_voltage_limited: bool,
    pub thermal_regulation: bool,
    pub minimum_system_voltage: bool,
    pub faults: ChargerFaults,
    /// Faults latched since the previous read. This is not a persistent event log.
    pub latched_faults: ChargerFaults,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerSnapshot {
    pub battery: PowerReading<BatteryState>,
    pub charger: PowerReading<ChargerState>,
}

impl PowerSnapshot {
    pub const INITIAL: Self = Self {
        battery: PowerReading::unavailable(PowerAvailability::Starting),
        charger: PowerReading::unavailable(PowerAvailability::Starting),
    };
}

pub trait Power {
    /// Return cached observations without I/O or a wait.
    /// Each source retains its own timestamp and availability.
    fn snapshot(&mut self, now_ms: u64) -> PowerSnapshot;
}
