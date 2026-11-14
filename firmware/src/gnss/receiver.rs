//! Bounded NMEA decoder and observation expiry for the GNSS adapter.
//!
//! # Design choices
//!
//! - Require a valid sentence before publishing receiver activity.
//! - Borrow large satellite storage so every receiver has configuration without a large construction temporary on the worker stack.
//!
//! See [LILYGO GNSS adapter](../../../docs/boards/lilygo-t5-s3-pro-gnss.md).

use super::{config::Config, gsv::Gsv, nmea::Sentence};
use paper_mesh_platform::location::{
    Coordinates, LocationAvailability, LocationObservation, LocationSnapshot, MotionObservation,
    ReceiverActivity, UtcObservation, UtcTime,
};
use paper_mesh_platform::time::Date;

/// Large satellite aggregation storage borrowed by a configured receiver.
/// Firmware keeps this in static memory to avoid constructing it on the worker stack.
pub struct ReceiverStorage {
    gsv: Gsv,
}

impl ReceiverStorage {
    pub const EMPTY: Self = Self { gsv: Gsv::EMPTY };
}

/// Bounded sentence decoder with required configuration and borrowed satellite storage.
pub struct Receiver<'a> {
    config: Config,
    sentence: Sentence,
    deadline_ms: u64,
    gsv: &'a mut Gsv,
    pub snapshot: LocationSnapshot,
}

impl<'a> Receiver<'a> {
    /// Borrow satellite storage and initialize a configured receiver without a large stack temporary.
    pub fn new(storage: &'a mut ReceiverStorage, config: Config) -> Self {
        storage.gsv.clear();
        Self {
            config,
            sentence: Sentence::default(),
            deadline_ms: config.silence_ms.get(),
            gsv: &mut storage.gsv,
            snapshot: LocationSnapshot::new(),
        }
    }

    /// Discard prior acquisition state while retaining configuration and storage ownership.
    pub fn reset(&mut self) {
        self.sentence.clear();
        self.deadline_ms = self.config.silence_ms.get();
        self.gsv.clear();
        self.snapshot = LocationSnapshot::new();
    }

    pub fn start_at(&mut self, now_ms: u64) {
        self.deadline_ms = now_ms.saturating_add(self.config.silence_ms.get());
    }

    pub fn deadline_ms(&self) -> u64 {
        self.gsv
            .next_deadline_ms()
            .map_or(self.deadline_ms, |time| time.min(self.deadline_ms))
    }

    /// A valid NMEA envelope establishes activity, even without a position fix.
    pub fn feed(&mut self, byte: u8, now_ms: u64) -> bool {
        if let Some(text) = self.sentence.feed(byte) {
            self.snapshot.availability = LocationAvailability::Ready;
            self.snapshot.activity = Some(ReceiverActivity {
                received_at_ms: now_ms,
                valid_messages: self
                    .snapshot
                    .activity
                    .map_or(1, |activity| activity.valid_messages.saturating_add(1)),
            });
            if let Some(observation) = parse_gga(text, now_ms) {
                self.snapshot.observation = Some(observation);
            }
            if let Some((motion, utc)) = parse_rmc(text, now_ms) {
                self.snapshot.motion = Some(motion);
                self.snapshot.utc = utc;
            }
            if text.as_bytes().get(2..5) == Some(b"GSV") {
                let mut fields = [""; 21];
                let mut count = 0;
                for field in text.split(',') {
                    if count == fields.len() {
                        break;
                    }
                    fields[count] = field;
                    count += 1;
                }
                if text.split(',').count() == count {
                    self.gsv.feed(&fields[..count], now_ms);
                } else {
                    self.gsv.feed(&fields[..1], now_ms);
                }
            }
            self.gsv.expire(now_ms);
            self.snapshot.satellites = self.gsv.observation();
            self.deadline_ms = now_ms.saturating_add(self.config.silence_ms.get());
            return true;
        }
        false
    }

    /// A UART error invalidates the observation and the partial sentence.
    pub fn uart_error(&mut self) {
        self.sentence.clear();
        if self.snapshot.availability == LocationAvailability::Ready {
            self.snapshot.availability = LocationAvailability::Fault;
        }
        self.snapshot.observation = None;
        self.snapshot.motion = None;
        self.snapshot.utc = None;
        self.gsv.clear();
        self.snapshot.satellites = None;
    }

    /// Expire observations after silence without changing transport configuration.
    pub fn advance(&mut self, now_ms: u64) -> bool {
        let expired = self.gsv.expire(now_ms);
        if expired {
            self.snapshot.satellites = self.gsv.observation();
        }
        if now_ms < self.deadline_ms {
            return expired;
        }
        self.sentence.clear();
        self.gsv.clear();
        self.snapshot.satellites = None;
        self.snapshot.availability = LocationAvailability::Unavailable;
        self.snapshot.observation = None;
        self.snapshot.motion = None;
        self.snapshot.utc = None;
        self.deadline_ms = now_ms.saturating_add(self.config.silence_ms.get());
        true
    }
}

fn parse_gga(text: &str, now_ms: u64) -> Option<LocationObservation> {
    let mut fields = [""; 15];
    let mut count = 0;
    for field in text.split(',') {
        *fields.get_mut(count)? = field;
        count += 1;
    }
    if count != fields.len()
        || !matches!(
            fields[0],
            "GPGGA" | "GNGGA" | "GLGGA" | "GAGGA" | "GBGGA" | "BDGGA" | "GQGGA"
        )
    {
        return None;
    }
    let quality = decimal(fields[6])?;
    if fields[6].len() != 1 || quality > 8 {
        return None;
    }
    let satellites_used = if fields[7].is_empty() {
        None
    } else {
        let count = decimal(fields[7])?;
        if count > 99 {
            return None;
        }
        Some(count as u8)
    };
    // Only satellite fixes are current positions. Estimated and simulated fixes are not.
    let coordinates = if matches!(quality, 1 | 2 | 4 | 5) {
        Some(Coordinates::new(
            coordinate(fields[2], fields[3], 2, 90, "N", "S")?,
            coordinate(fields[4], fields[5], 3, 180, "E", "W")?,
        )?)
    } else {
        None
    };
    Some(LocationObservation {
        observed_at_ms: now_ms,
        coordinates,
        elevation_mm: if coordinates.is_some() && fields[10] == "M" {
            meters_to_mm(fields[9])
        } else {
            None
        },
        satellites_used,
    })
}

fn meters_to_mm(text: &str) -> Option<i32> {
    let negative = text.starts_with('-');
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((_, "")) => return None,
        Some(parts) => parts,
        None => (unsigned, ""),
    };
    let meters = decimal(whole)?;
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = fraction.len().min(3);
    let millimeters = if digits == 0 {
        0
    } else {
        decimal(&fraction[..digits])? * 10_u32.pow(3 - digits as u32)
    };
    let magnitude = i64::from(meters) * 1_000 + i64::from(millimeters);
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

fn parse_rmc(text: &str, now_ms: u64) -> Option<(MotionObservation, Option<UtcObservation>)> {
    if text.as_bytes().get(2..5) != Some(b"RMC") {
        return None;
    }
    let mut fields = [""; 14];
    let mut count = 0;
    for field in text.split(',') {
        *fields.get_mut(count)? = field;
        count += 1;
    }
    if !(12..=14).contains(&count) || !matches!(fields[2], "A" | "D" | "V") {
        return None;
    }
    // The optional mode excludes estimated, manual, and simulated data.
    let valid = matches!(fields[2], "A" | "D")
        && (count == 12 || matches!(fields[12], "A" | "D" | "F" | "R"));
    let speed_mm_s = valid
        .then(|| unsigned_thousandths(fields[7]))
        .flatten()
        .map(|knots| ((u64::from(knots) * 1_852 + 1_800) / 3_600) as u32);
    let course_millidegrees = valid
        .then(|| unsigned_thousandths(fields[8]))
        .flatten()
        .filter(|course| *course < 360_000);
    let utc = valid
        .then(|| utc_time(fields[1]))
        .flatten()
        .map(|time| UtcObservation {
            observed_at_ms: now_ms,
            time,
            date: utc_date(fields[9]),
        });
    Some((
        MotionObservation {
            observed_at_ms: now_ms,
            speed_mm_s,
            course_millidegrees,
        },
        utc,
    ))
}

fn utc_date(text: &str) -> Option<Date> {
    if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // RMC has two year digits. This backend uses the years 2000 through 2099.
    Date::from_calendar_date(
        2000 + decimal(&text[4..])? as i32,
        (decimal(&text[2..4])? as u8).try_into().ok()?,
        decimal(&text[..2])? as u8,
    )
    .ok()
}

fn unsigned_thousandths(text: &str) -> Option<u32> {
    let (whole, fraction) = match text.split_once('.') {
        Some((_, "")) => return None,
        Some(parts) => parts,
        None => (text, ""),
    };
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = fraction.len().min(3);
    let fractional = if digits == 0 {
        0
    } else {
        decimal(&fraction[..digits])? * 10_u32.pow(3 - digits as u32)
    };
    decimal(whole)?.checked_mul(1_000)?.checked_add(fractional)
}

fn utc_time(text: &str) -> Option<UtcTime> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.len() != 6
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (text.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    UtcTime::new(
        decimal(&whole[..2])? as u8,
        decimal(&whole[2..4])? as u8,
        decimal(&whole[4..])? as u8,
    )
}

fn coordinate(
    text: &str,
    hemisphere: &str,
    degrees_len: usize,
    limit: u32,
    positive: &str,
    negative: &str,
) -> Option<i32> {
    if !text.is_ascii() {
        return None;
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.len() != degrees_len + 2 || fraction.len() > 7 {
        return None;
    }
    let degrees = decimal(&whole[..degrees_len])?;
    let minutes = decimal(&whole[degrees_len..])?;
    let scale = 10_u32.pow(fraction.len() as u32);
    let fractional = if fraction.is_empty() {
        0
    } else {
        decimal(fraction)?
    };
    if minutes >= 60 || degrees > limit || (degrees == limit && (minutes != 0 || fractional != 0)) {
        return None;
    }
    let minute_units = u64::from(minutes) * u64::from(scale) + u64::from(fractional);
    let divisor = 60 * u64::from(scale);
    let value = degrees * 1_000_000 + ((minute_units * 1_000_000 + divisor / 2) / divisor) as u32;
    let sign = if hemisphere == positive {
        1
    } else if hemisphere == negative {
        -1
    } else {
        return None;
    };
    Some(sign * value as i32)
}

pub(super) fn decimal(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    text.bytes().try_fold(0_u32, |value, byte| {
        if !byte.is_ascii_digit() {
            return None;
        }
        value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))
    })
}
