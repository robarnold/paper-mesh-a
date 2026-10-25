//! BQ25896 battery isolation after the runtime stops device services.

use embedded_hal::i2c::I2c;

const CHARGER: u8 = 0x6b;

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    ExternalPower,
    Verification,
}

fn read<I: I2c>(bus: &mut I, register: u8) -> Result<u8, Error<I::Error>> {
    let mut byte = [0];
    bus.write_read(CHARGER, &[register], &mut byte)
        .map_err(Error::Bus)?;
    Ok(byte[0])
}

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

pub fn cut<I: I2c>(bus: &mut I) -> Result<(), Error<I::Error>> {
    let old = read(bus, 0x09)?;
    check(bus)?;
    // Preserve JEITA, safety timer, and BATFET reset settings.
    // Clear the pump and force-detection commands. Select immediate isolation.
    bus.write(CHARGER, &[0x09, (old & 0x54) | 0x20])
        .map_err(Error::Bus)
}

pub fn cancel<I: I2c>(bus: &mut I) -> Result<(), Error<I::Error>> {
    // If the CPU still runs, remove the cut request before services resume.
    let old = read(bus, 0x09)?;
    bus.write(CHARGER, &[0x09, old & 0x54])
        .map_err(Error::Bus)?;
    if read(bus, 0x09)? & 0x28 != 0 {
        return Err(Error::Verification);
    }
    Ok(())
}
