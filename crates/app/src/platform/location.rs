//! Location observations use the application's monotonic epoch.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationAvailability {
    Starting,
    Ready,
    Unavailable,
    Fault,
}

/// WGS84 coordinates in millionths of a degree. Zero is a valid coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coordinates {
    latitude_microdegrees: i32,
    longitude_microdegrees: i32,
}

impl Coordinates {
    pub fn new(latitude_microdegrees: i32, longitude_microdegrees: i32) -> Option<Self> {
        if !(-90_000_000..=90_000_000).contains(&latitude_microdegrees)
            || !(-180_000_000..=180_000_000).contains(&longitude_microdegrees)
        {
            return None;
        }
        Some(Self {
            latitude_microdegrees,
            longitude_microdegrees,
        })
    }

    pub fn latitude_microdegrees(self) -> i32 {
        self.latitude_microdegrees
    }

    pub fn longitude_microdegrees(self) -> i32 {
        self.longitude_microdegrees
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocationObservation {
    /// Local receipt time. This value is not UTC or the source measurement time.
    pub observed_at_ms: u64,
    /// None means that the provider has no valid position fix.
    pub coordinates: Option<Coordinates>,
    /// Elevation above mean sea level in millimeters. None means unavailable.
    pub elevation_mm: Option<i32>,
    /// Number of satellites in the solution. None means unknown.
    pub satellites_used: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionObservation {
    pub observed_at_ms: u64,
    /// Speed over ground in millimeters per second. Zero is valid.
    pub speed_mm_s: Option<u32>,
    /// Course over ground, clockwise from true north, in thousandths of a degree.
    /// Values must be less than 360000. None means unavailable.
    pub course_millidegrees: Option<u32>,
}

/// UTC time of day. This value does not specify a date or clock accuracy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtcTime {
    hour: u8,
    minute: u8,
    second: u8,
}

impl UtcTime {
    pub fn new(hour: u8, minute: u8, second: u8) -> Option<Self> {
        (hour < 24 && minute < 60 && second < 60).then_some(Self {
            hour,
            minute,
            second,
        })
    }

    pub fn hour(self) -> u8 {
        self.hour
    }
    pub fn minute(self) -> u8 {
        self.minute
    }
    pub fn second(self) -> u8 {
        self.second
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtcObservation {
    pub observed_at_ms: u64,
    pub time: UtcTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiverActivity {
    pub received_at_ms: u64,
    /// Number of messages with a valid envelope and checksum in this session.
    pub valid_messages: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SatelliteSystem {
    Gps,
    Glonass,
    Galileo,
    BeiDou,
    Qzss,
    Navic,
}

impl SatelliteSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gps => "GPS",
            Self::Glonass => "GLO",
            Self::Galileo => "GAL",
            Self::BeiDou => "BDS",
            Self::Qzss => "QZS",
            Self::Navic => "NAV",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SatelliteSignal {
    pub system: SatelliteSystem,
    pub id: u16,
    /// Carrier-to-noise density in dB-Hz. None means no reported signal level.
    pub cn0_db_hz: Option<u8>,
}

pub const DISPLAY_SATELLITES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SatelliteObservation {
    /// Oldest receipt time among the complete reports in this summary.
    pub observed_at_ms: u64,
    pub satellites_in_view: u16,
    pub satellites_with_signal: u16,
    /// At most eight satellites, in descending signal order.
    pub strongest: [Option<SatelliteSignal>; DISPLAY_SATELLITES],
    /// Counts are lower bounds when the provider exceeds its storage capacity.
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocationSnapshot {
    pub availability: LocationAvailability,
    pub observation: Option<LocationObservation>,
    pub motion: Option<MotionObservation>,
    pub utc: Option<UtcObservation>,
    pub activity: Option<ReceiverActivity>,
    pub satellites: Option<SatelliteObservation>,
}

impl LocationSnapshot {
    pub const fn new() -> Self {
        Self {
            availability: LocationAvailability::Starting,
            observation: None,
            motion: None,
            utc: None,
            activity: None,
            satellites: None,
        }
    }
}

impl Default for LocationSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// The provider returns its latest observation without a wait or an allocation.
/// Repeated reads must keep the original receipt time.
pub trait Location {
    fn snapshot(&mut self, now_ms: u64) -> LocationSnapshot;
}
