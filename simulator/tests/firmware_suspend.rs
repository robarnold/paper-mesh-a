//! Native checks for core coordination and expander preparation.

#[path = "../../firmware/src/suspend/expander.rs"]
mod expander;
#[path = "../../firmware/src/suspend/park.rs"]
mod park;

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
            return Err(ErrorKind::Bus);
        }
        match operations {
            [Operation::Write(register), Operation::Read(values)] => {
                assert_eq!(register.len(), 1);
                let start = register[0] as usize;
                values.copy_from_slice(&self.registers[start..start + values.len()]);
            }
            [Operation::Write(bytes)] => {
                assert_eq!(bytes.len(), 2);
                // Unconnected pins must have low latches before they drive the pins.
                if bytes[0] == 6 {
                    assert_eq!(self.registers[2] & 0xfe, 0);
                }
                self.registers[bytes[0] as usize] = bytes[1];
            }
            _ => panic!("unexpected I2C transaction"),
        }
        Ok(())
    }
}

#[test]
fn unused_pins_are_stable_without_changing_supply_or_display_controls() {
    for output in [0, 1, 0xaa, 0xff] {
        for direction in [0, 1, 0xfe, 0xff] {
            let original = [0x12, 0x84, output, 0x5b, 0x35, 0, direction, 0xc4];
            for fail_at in [None, Some(1), Some(2), Some(3), Some(4)] {
                let mut bus = Expander {
                    registers: original,
                    transactions: 0,
                    fail_at,
                };
                assert_eq!(expander::quiet_unused(&mut bus).is_err(), fail_at.is_some());
                bus.fail_at = None;
                expander::quiet_unused(&mut bus).unwrap();
                expander::quiet_unused(&mut bus).unwrap();
                let mut expected = original;
                expected[2] &= 1;
                expected[6] &= 1;
                assert_eq!(bus.registers, expected);
            }
        }
    }
}

#[test]
fn wake_capture_reads_both_ports_and_reports_a_failed_read() {
    let mut bus = Expander {
        registers: [0x01, 0x84, 0, 0, 0, 0, 0, 0],
        transactions: 0,
        fail_at: Some(1),
    };
    assert!(expander::inputs(&mut bus).is_err());
    assert_eq!(expander::inputs(&mut bus).unwrap(), [0x01, 0x84]);
}
