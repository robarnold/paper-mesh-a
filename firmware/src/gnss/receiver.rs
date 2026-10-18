//! Bounded NMEA decoder and UART baud selection for the GNSS adapter.

use super::gsv::Gsv;
use paper_mesh_platform::location::{
    Coordinates, LocationAvailability, LocationObservation, LocationSnapshot, MotionObservation,
    ReceiverActivity, UtcObservation, UtcTime,
};

pub const PROBE_MS: u64 = 4_000;
pub const SILENCE_MS: u64 = 10_000;
const BAUD_RATES: [u32; 2] = [9_600, 38_400];

pub struct Receiver {
    frame: [u8; 128],
    length: usize,
    collecting: bool,
    rate: usize,
    deadline_ms: u64,
    gsv: Gsv,
    pub snapshot: LocationSnapshot,
}

impl Receiver {
    pub const INITIAL: Self = Self {
        frame: [0; 128],
        length: 0,
        collecting: false,
        rate: 0,
        deadline_ms: PROBE_MS,
        gsv: Gsv::EMPTY,
        snapshot: LocationSnapshot::new(),
    };

    pub fn start_at(&mut self, now_ms: u64) {
        self.deadline_ms = now_ms.saturating_add(PROBE_MS);
    }

    pub fn baud(&self) -> u32 {
        BAUD_RATES[self.rate]
    }
    pub fn deadline_ms(&self) -> u64 {
        self.gsv
            .next_deadline_ms()
            .map_or(self.deadline_ms, |time| time.min(self.deadline_ms))
    }

    /// A valid NMEA envelope selects the baud rate, even without a position fix.
    pub fn feed(&mut self, byte: u8, now_ms: u64) -> bool {
        if byte == b'$' {
            self.length = 0;
            self.collecting = true;
        } else if self.collecting && byte == b'\n' {
            self.collecting = false;
            if let Some(text) = message(&self.frame[..self.length]) {
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
                self.deadline_ms = now_ms.saturating_add(SILENCE_MS);
                return true;
            }
        } else if self.collecting {
            if self.length == self.frame.len() {
                self.collecting = false;
            } else {
                self.frame[self.length] = byte;
                self.length += 1;
            }
        }
        false
    }

    /// A UART error invalidates the observation and the partial sentence.
    pub fn uart_error(&mut self) {
        self.collecting = false;
        self.length = 0;
        if self.snapshot.availability == LocationAvailability::Ready {
            self.snapshot.availability = LocationAvailability::Fault;
        }
        self.snapshot.observation = None;
        self.snapshot.motion = None;
        self.snapshot.utc = None;
        self.gsv.clear();
        self.snapshot.satellites = None;
    }

    /// After silence, both baud rates remain available for automatic recovery.
    pub fn advance(&mut self, now_ms: u64) -> bool {
        let expired = self.gsv.expire(now_ms);
        if expired {
            self.snapshot.satellites = self.gsv.observation();
        }
        if now_ms < self.deadline_ms {
            return expired;
        }
        self.rate = (self.rate + 1) % BAUD_RATES.len();
        self.collecting = false;
        self.length = 0;
        self.gsv.clear();
        self.snapshot.satellites = None;
        if self.rate == 0 || self.snapshot.availability != LocationAvailability::Starting {
            self.snapshot.availability = LocationAvailability::Unavailable;
            self.snapshot.observation = None;
            self.snapshot.motion = None;
            self.snapshot.utc = None;
        }
        self.deadline_ms = now_ms.saturating_add(PROBE_MS);
        true
    }
}

fn message(frame: &[u8]) -> Option<&str> {
    let frame = frame.strip_suffix(b"\r").unwrap_or(frame);
    let star = frame.iter().position(|byte| *byte == b'*')?;
    if star + 3 != frame.len() {
        return None;
    }
    let checksum = hex(frame[star + 1])? * 16 + hex(frame[star + 2])?;
    if frame[..star].iter().fold(0, |sum, byte| sum ^ byte) != checksum {
        return None;
    }
    let text = core::str::from_utf8(&frame[..star]).ok()?;
    let header = text.split(',').next()?;
    if header.len() != 5
        || !header.bytes().all(|byte| byte.is_ascii_uppercase())
        || !matches!(
            &header[..2],
            "GP" | "GN" | "GL" | "GA" | "GB" | "BD" | "GQ" | "GI"
        )
        || !text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
        || !text.contains(',')
    {
        return None;
    }
    Some(text)
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

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
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
