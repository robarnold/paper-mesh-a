//! Verify shared rail register isolation and recovery after bus failures.
//!
//! # Design choices
//!
//! - Exercise the physical rail independently of GNSS decoding and lifecycle state.
//!
//! See [auxiliary rail ownership](../../../../docs/boards/lilygo-t5-s3-pro-power.md#auxiliary-rail).

use super::{disable, enable};
use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};

struct Expander {
    registers: [u8; 8],
    transactions: usize,
    fail_at: Option<usize>,
}

impl ErrorType for Expander {
    type Error = ErrorKind;
}

impl I2c for Expander {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        assert_eq!(address, 0x20);
        self.transactions += 1;
        if self.fail_at == Some(self.transactions) {
            return Err(ErrorKind::Other);
        }
        match operations {
            [Operation::Write(register), Operation::Read(value)] => {
                value[0] = self.registers[register[0] as usize];
            }
            [Operation::Write(bytes)] => {
                self.registers[bytes[0] as usize] = bytes[1];
                // Enabling the output must follow the high output latch.
                if bytes[0] == 6 && self.registers[6] & 1 == 0 {
                    assert_eq!(self.registers[2] & 1, 1);
                }
            }
            _ => panic!("unexpected I2C transaction"),
        }
        Ok(())
    }
}

// Enabling the rail preserves unrelated outputs and permits retry after each partial failure.
#[test]
fn power_enable_preserves_display_controls_and_recovers_after_each_bus_failure() {
    let original = [0x12, 0x34, 0xaa, 0x5b, 0xcd, 0xef, 0xff, 0xc4];
    for fail_at in [None, Some(1), Some(2), Some(3), Some(4)] {
        let mut bus = Expander {
            registers: original,
            transactions: 0,
            fail_at,
        };
        assert_eq!(enable(&mut bus).is_err(), fail_at.is_some());
        bus.fail_at = None;
        enable(&mut bus).unwrap();
        enable(&mut bus).unwrap();
        let mut expected = original;
        expected[2] |= 1;
        expected[6] &= !1;
        assert_eq!(bus.registers, expected);
    }
}

// Disabling the rail preserves unrelated outputs and exposes each bus failure.
#[test]
fn suspend_supply_shutdown_preserves_other_outputs_and_propagates_errors() {
    let original = [0x12, 0x34, 0xab, 0x5b, 0xcd, 0xef, 0xfe, 0xc4];
    for fail_at in [None, Some(1), Some(2)] {
        let mut bus = Expander {
            registers: original,
            transactions: 0,
            fail_at,
        };
        assert_eq!(disable(&mut bus).is_err(), fail_at.is_some());
        bus.fail_at = None;
        disable(&mut bus).unwrap();
        let mut expected = original;
        expected[2] &= !1;
        assert_eq!(bus.registers, expected);
        enable(&mut bus).unwrap();
        assert_eq!(bus.registers, original);
    }
}
