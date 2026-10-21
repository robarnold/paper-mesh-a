//! Stable unused pins on the H752-01 control expander.

/// Hold unconnected IO0_1 through IO0_7 low without changing the auxiliary supply.
pub fn quiet_unused<I: embedded_hal::i2c::I2c>(bus: &mut I) -> Result<(), I::Error> {
    let mut value = [0];
    // PCA9535 has no input pull resistors. Set each latch before its direction.
    bus.write_read(0x20, &[2], &mut value)?;
    bus.write(0x20, &[2, value[0] & 1])?;
    bus.write_read(0x20, &[6], &mut value)?;
    bus.write(0x20, &[6, value[0] & 1])
}

pub fn inputs<I: embedded_hal::i2c::I2c>(bus: &mut I) -> Result<[u8; 2], I::Error> {
    let mut ports = [0; 2];
    bus.write_read(0x20, &[0], &mut ports)?;
    Ok(ports)
}
