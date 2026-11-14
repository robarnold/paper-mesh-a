//! BQ25896 battery isolation after the runtime stops device services.
//!
//! # Design choices
//!
//! - Recheck the supply before battery isolation because external power can change
//!   after preparation.
//!
//! See [System power-off](../../../docs/adapters/bq25896.md#battery-isolation).

use embedded_hal::i2c::I2c;

use crate::I2C_ADDRESS;

/// Access failure, active external supply or boost, or unconfirmed startup restoration.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    ExternalPower,
    Verification,
}

fn read<I: I2c>(bus: &mut I, register: u8) -> Result<u8, Error<I::Error>> {
    let mut byte = [0];
    bus.write_read(I2C_ADDRESS, &[register], &mut byte)
        .map_err(Error::Bus)?;
    Ok(byte[0])
}

/// Refuse battery isolation while input power or boost is active. This does not reserve that condition.
pub fn check<I: I2c>(bus: &mut I) -> Result<(), Error<I::Error>> {
    // Check live input presence, charger status, and boost configuration.
    // A USB data disconnect does not establish that VBUS is absent.
    let vbus = read(bus, 0x11)?;
    let status = read(bus, 0x0b)?;
    let control = read(bus, 0x03)?;
    if vbus & 0x80 != 0 || status & 0xe4 != 0 || control & 0x20 != 0 {
        return Err(Error::ExternalPower);
    }
    Ok(())
}

/// Recheck input power and request immediate battery isolation once.
/// The caller must stop consumers and hold the bus for the entire sequence.
/// An access error can leave the cut state uncertain. Success confirms only the register write.
pub fn cut<I: I2c>(bus: &mut I) -> Result<(), Error<I::Error>> {
    let old = read(bus, 0x09)?;
    check(bus)?;
    // Preserve JEITA, safety timer, and BATFET reset settings.
    // Clear the pump and force-detection commands. Select immediate isolation.
    bus.write(I2C_ADDRESS, &[0x09, (old & 0x54) | 0x20])
        .map_err(Error::Bus)
}

/// Clear retained isolation commands and verify BATFET controls before consumers start.
/// Preserve unrelated configuration. The caller must hold the bus for the entire sequence.
pub fn initialize<I: I2c>(bus: &mut I) -> Result<(), Error<I::Error>> {
    // Charger state survives a processor reset. Clear retained cut requests before workers start.
    let old = read(bus, 0x09)?;
    bus.write(I2C_ADDRESS, &[0x09, old & 0x54])
        .map_err(Error::Bus)?;
    if read(bus, 0x09)? & 0x28 != 0 {
        return Err(Error::Verification);
    }
    Ok(())
}
