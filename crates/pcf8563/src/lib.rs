//! PCF8563 calendar access through the shared I2C bus.
//!
//! # Design choices
//!
//! - Read the complete calendar together and reject voltage-loss indications so a
//!   partial or invalid date cannot become UTC.
//!
//! See [Calendar access](../../../docs/adapters/rtc.md#calendar-access).

#![no_std]

use embedded_hal::i2c::I2c;
use paper_mesh_platform::rtc::{InvalidTimeReason, Rtc, RtcError};
use paper_mesh_platform::time::{Date, Time, UtcDateTime};

const ADDRESS: u8 = 0x51;
const STOP: u8 = 0x20;

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    UnsupportedYear,
}

/// Read all calendar fields in one transaction to prevent mixed rollover values.
pub fn read<I: I2c>(bus: &mut I) -> Result<Option<UtcDateTime>, Error<I::Error>> {
    let mut registers = [0; 9];
    bus.write_read(ADDRESS, &[0], &mut registers)
        .map_err(Error::Bus)?;
    Ok(decode(registers))
}

fn decode(registers: [u8; 9]) -> Option<UtcDateTime> {
    // Test mode, STOP, or voltage loss invalidates the calendar.
    // C=0 selects 2000..2099. C=1 is outside this backend's supported range.
    if registers[0] & 0xa8 != 0 || registers[2] & 0x80 != 0 || registers[7] & 0x80 != 0 {
        return None;
    }
    if registers[6] & 7 > 6 {
        return None;
    }
    let date = Date::from_calendar_date(
        2000 + i32::from(bcd(registers[8])?),
        bcd(registers[7] & 0x1f)?.try_into().ok()?,
        bcd(registers[5] & 0x3f)?,
    )
    .ok()?;
    let time = Time::from_hms(
        bcd(registers[4] & 0x3f)?,
        bcd(registers[3] & 0x7f)?,
        bcd(registers[2] & 0x7f)?,
    )
    .ok()?;
    Some(UtcDateTime::new(date, time))
}

/// Stop the counter, replace the complete calendar, then start normal operation.
/// A failed calendar write keeps STOP set until a later complete correction.
pub fn set<I: I2c>(bus: &mut I, datetime: UtcDateTime) -> Result<(), Error<I::Error>> {
    if !(2000..=2099).contains(&datetime.year()) {
        return Err(Error::UnsupportedYear);
    }
    bus.write(ADDRESS, &[0, STOP]).map_err(Error::Bus)?;
    bus.write(
        ADDRESS,
        &[
            2,
            encode(datetime.second()),
            encode(datetime.minute()),
            encode(datetime.hour()),
            encode(datetime.day()),
            datetime.weekday().number_days_from_sunday(),
            encode(datetime.month() as u8),
            encode((datetime.year() - 2000) as u8),
        ],
    )
    .map_err(Error::Bus)?;
    bus.write(ADDRESS, &[0, 0]).map_err(Error::Bus)
}

fn bcd(value: u8) -> Option<u8> {
    let tens = value >> 4;
    let units = value & 15;
    (tens < 10 && units < 10).then_some(tens * 10 + units)
}

fn encode(value: u8) -> u8 {
    (value / 10) << 4 | (value % 10)
}

/// A PCF8563 RTC with an owned I2C handle or a borrowed bus.
pub struct Pcf8563<I> {
    bus: I,
}

impl<I: I2c> Pcf8563<I> {
    /// Store the bus handle without I/O or device initialization.
    pub fn new(bus: I) -> Self {
        Self { bus }
    }
}

impl<I: I2c> Rtc for Pcf8563<I> {
    type AccessError = I::Error;

    fn read_utc(&mut self) -> Result<UtcDateTime, RtcError<Self::AccessError>> {
        read(&mut self.bus)
            .map_err(map_error)?
            .ok_or(RtcError::InvalidTime(InvalidTimeReason::InvalidCalendar))
    }

    fn set_utc(&mut self, utc: UtcDateTime) -> Result<(), RtcError<Self::AccessError>> {
        set(&mut self.bus, utc).map_err(map_error)
    }
}

fn map_error<E>(error: Error<E>) -> RtcError<E> {
    match error {
        Error::Bus(error) => RtcError::Access(error),
        Error::UnsupportedYear => RtcError::UnsupportedDate,
    }
}
