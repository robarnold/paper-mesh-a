//! Bounded value formatting with explicit units.

use core::fmt::Write;
use heapless::String;

use crate::location::{SatelliteSystem, UtcTime};

/// Display label for a satellite system.
pub fn satellite_label(system: SatelliteSystem) -> &'static str {
    match system {
        SatelliteSystem::Gps => "GPS",
        SatelliteSystem::Glonass => "GLO",
        SatelliteSystem::Galileo => "GAL",
        SatelliteSystem::BeiDou => "BDS",
        SatelliteSystem::Qzss => "QZS",
        SatelliteSystem::Navic => "NAV",
    }
}

/// Seconds, or an unavailable marker.
pub fn elapsed_seconds(age: Option<u64>) -> String<24> {
    let mut label = String::new();
    if let Some(age) = age {
        let _ = write!(label, "{age}");
    } else {
        let _ = label.push_str("--");
    }
    label
}

/// Millimeters per second as km/h with one decimal place.
pub fn speed_kmh(value: Option<u32>) -> String<24> {
    let mut label = String::new();
    if let Some(speed) = value {
        let tenths = (u64::from(speed) * 36 + 500) / 1_000;
        let _ = write!(label, "{}.{:01}", tenths / 10, tenths % 10);
    } else {
        let _ = label.push_str("--");
    }
    label
}

/// Thousandths of a degree as a true course and an eight-point compass direction.
pub fn course_true_degrees(value: Option<u32>) -> String<24> {
    let mut label = String::new();
    if let Some(course) = value.filter(|course| *course < 360_000) {
        let directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
        let direction = directions[((course + 22_500) / 45_000 % 8) as usize];
        let tenths = ((course + 50) / 100) % 3_600;
        let _ = write!(
            label,
            "{direction} {}.{} deg true",
            tenths / 10,
            tenths % 10
        );
    } else {
        let _ = label.push_str("--");
    }
    label
}

/// UTC time of day as HH:MM:SS, or an unavailable marker.
pub fn utc_time(value: Option<UtcTime>) -> String<24> {
    let mut label = String::new();
    if let Some(time) = value {
        let _ = write!(
            label,
            "{:02}:{:02}:{:02}",
            time.hour(),
            time.minute(),
            time.second()
        );
    } else {
        let _ = label.push_str("--:--:--");
    }
    label
}

/// Signed millimeters as meters with one decimal place.
pub fn elevation_meters(value: Option<i32>) -> String<24> {
    let mut label = String::new();
    if let Some(value) = value {
        let tenths = (value.unsigned_abs() + 50) / 100;
        let _ = write!(
            label,
            "{}{}.{:01}",
            if value < 0 && tenths != 0 { "-" } else { "" },
            tenths / 10,
            tenths % 10
        );
    } else {
        let _ = label.push_str("--");
    }
    label
}

/// Signed millionths of a degree with six decimal places.
pub fn coordinate_degrees(value: Option<i32>) -> String<24> {
    let mut label = String::new();
    if let Some(value) = value {
        let magnitude = value.unsigned_abs();
        let _ = write!(
            label,
            "{}{}.{:06}",
            if value < 0 { "-" } else { "" },
            magnitude / 1_000_000,
            magnitude % 1_000_000
        );
    } else {
        let _ = label.push_str("--");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_labels_keep_the_sign_below_one_degree() {
        assert_eq!(coordinate_degrees(Some(-500_001)), "-0.500001");
        assert_eq!(coordinate_degrees(Some(0)), "0.000000");
        assert_eq!(coordinate_degrees(Some(180_000_000)), "180.000000");
        assert_eq!(coordinate_degrees(None), "--");
    }

    #[test]
    fn elevation_labels_round_to_tenths_and_preserve_missing_values() {
        assert_eq!(elevation_meters(Some(75_700)), "75.7");
        assert_eq!(elevation_meters(Some(-430_500)), "-430.5");
        assert_eq!(elevation_meters(Some(-150)), "-0.2");
        assert_eq!(elevation_meters(Some(-1)), "0.0");
        assert_eq!(elevation_meters(Some(0)), "0.0");
        assert_eq!(elevation_meters(Some(i32::MIN)), "-2147483.6");
        assert_eq!(elevation_meters(None), "--");
    }

    #[test]
    fn motion_and_clock_labels_keep_zero_missing_and_direction_boundaries() {
        assert_eq!(speed_kmh(Some(0)), "0.0");
        assert_eq!(speed_kmh(Some(5_144)), "18.5");
        assert_eq!(speed_kmh(Some(u32::MAX)), "15461882.3");
        assert_eq!(speed_kmh(None), "--");
        for (course, expected) in [
            (0, "N 0.0 deg true"),
            (22_499, "N 22.5 deg true"),
            (22_500, "NE 22.5 deg true"),
            (90_000, "E 90.0 deg true"),
            (180_000, "S 180.0 deg true"),
            (270_000, "W 270.0 deg true"),
            (315_000, "NW 315.0 deg true"),
            (359_999, "N 0.0 deg true"),
            (360_000, "--"),
        ] {
            assert_eq!(course_true_degrees(Some(course)), expected);
        }
        assert_eq!(course_true_degrees(None), "--");
        assert_eq!(utc_time(UtcTime::new(0, 0, 0)), "00:00:00");
        assert_eq!(utc_time(UtcTime::new(23, 59, 59)), "23:59:59");
        assert_eq!(utc_time(None), "--:--:--");
        assert_eq!(UtcTime::new(24, 0, 0), None);
        assert_eq!(UtcTime::new(0, 60, 0), None);
        assert_eq!(UtcTime::new(0, 0, 60), None);
    }
}
