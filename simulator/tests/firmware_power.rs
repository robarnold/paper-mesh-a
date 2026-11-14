//! Verify independent gauge and charger failures through their public crate APIs.
//!
//! # Design choices
//!
//! - Use scripted bus observations to check power decoding when one sensor supplies
//!   incomplete or invalid data.
//!
//! See [LILYGO power control and observations](../../docs/boards/lilygo-t5-s3-pro-power.md).

use paper_mesh_bq25896::observation as device;

use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};

struct Bus {
    gauge: [u8; 64],
    charger: [u8; 21],
    latched: u8,
    faults_read: usize,
    transactions: usize,
    fail_at: Option<usize>,
}

impl Bus {
    fn new() -> Self {
        let mut bus = Self {
            gauge: [0; 64],
            charger: [0; 21],
            latched: 0x10,
            faults_read: 0,
            transactions: 0,
            fail_at: None,
        };
        for (register, value) in [
            (0x0a, 9),
            (0x3a, 0x20),
            (0x08, 3890),
            (0x0c, (-180_i16) as u16),
            (0x14, (-175_i16) as u16),
            (0x24, (-700_i16) as u16),
            (0x28, 2989),
            (0x2c, 68),
            (0x2e, 94),
            (0x10, 1918),
            (0x12, 2820),
            (0x3c, 3000),
            (0x2a, 42),
            (0x16, 639),
            (0x18, u16::MAX),
        ] {
            bus.word(register, value);
        }
        bus.charger[0x0b] = 0x36;
        bus.charger[0x00] = 0x48;
        bus.charger[0x03] = 0x1a;
        bus.charger[0x04] = 16;
        bus.charger[0x05] = 0x11;
        bus.charger[0x06] = 0x5e;
        bus.charger[0x0d] = 18;
        bus
    }

    fn word(&mut self, register: usize, value: u16) {
        self.gauge[register..register + 2].copy_from_slice(&value.to_le_bytes());
    }
}

impl ErrorType for Bus {
    type Error = ErrorKind;
}
impl I2c for Bus {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.transactions += 1;
        if self.fail_at == Some(self.transactions) {
            return Err(ErrorKind::Other);
        }
        let [Operation::Write(pointer), Operation::Read(bytes)] = operations else {
            panic!("power reads must not write configuration");
        };
        assert_eq!(pointer.len(), 1, "only a register pointer can be written");
        let register = pointer[0] as usize;
        match address {
            0x55 => {
                assert_eq!(bytes.len(), 2);
                bytes.copy_from_slice(&self.gauge[register..register + 2]);
            }
            0x6b => {
                assert_eq!(bytes.len(), 1);
                bytes[0] = if register == 0x0c {
                    self.faults_read += 1;
                    if self.faults_read == 1 {
                        self.latched
                    } else {
                        self.charger[register]
                    }
                } else {
                    self.charger[register]
                };
            }
            _ => panic!("unexpected address"),
        }
        Ok(())
    }
}

// Failure in either device leaves the other readable, and a subsequent complete read can recover.
#[test]
fn every_bus_failure_discards_the_partial_sample_and_next_read_recovers() {
    let mut probe = Bus::new();
    paper_mesh_bq27220::battery(&mut probe).unwrap();
    let battery_reads = probe.transactions;
    for fail in 1..=battery_reads {
        let mut bus = Bus::new();
        bus.fail_at = Some(fail);
        assert!(paper_mesh_bq27220::battery(&mut bus).is_err());
        assert!(device::read(&mut bus).is_ok());
        bus.fail_at = None;
        assert!(paper_mesh_bq27220::battery(&mut bus).is_ok());
    }
    let mut probe = Bus::new();
    device::read(&mut probe).unwrap();
    for fail in 1..=probe.transactions {
        let mut bus = Bus::new();
        bus.fail_at = Some(fail);
        assert!(device::read(&mut bus).is_err());
        assert!(paper_mesh_bq27220::battery(&mut bus).is_ok());
        bus.fail_at = None;
        assert!(device::read(&mut bus).is_ok());
    }
}
