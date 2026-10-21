//! Auxiliary supply control through PCA9535 port 0.

pub fn enable<I: embedded_hal::i2c::I2c>(bus: &mut I) -> Result<(), I::Error> {
    let mut value = [0];
    // Set the output latch before the output direction.
    // Preserve other port 0 bits and all port 1 display controls.
    bus.write_read(0x20, &[2], &mut value)?;
    bus.write(0x20, &[2, value[0] | 1])?;
    bus.write_read(0x20, &[6], &mut value)?;
    bus.write(0x20, &[6, value[0] & !1])?;
    Ok(())
}

pub fn disable<I: embedded_hal::i2c::I2c>(bus: &mut I) -> Result<(), I::Error> {
    let mut value = [0];
    bus.write_read(0x20, &[2], &mut value)?;
    bus.write(0x20, &[2, value[0] & !1])
}
