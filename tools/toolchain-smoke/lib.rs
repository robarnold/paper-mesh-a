//! Infrastructure smoke test; this is not device firmware.
#![no_std]

pub fn checksum(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0, |sum, byte| sum.wrapping_add(*byte as u32))
}
