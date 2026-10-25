#[path = "../../firmware/src/power/ship.rs"]
mod ship;

use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};

#[derive(Default)]
struct Bus {
    registers: [u8; 21],
    writes: Vec<(u8, u8)>,
    calls: usize,
    fail_at: Option<usize>,
    insert_at: Option<usize>,
    ignore_write: bool,
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
        assert_eq!(address, 0x6b);
        self.calls += 1;
        if self.fail_at == Some(self.calls) {
            return Err(ErrorKind::Other);
        }
        if self.insert_at == Some(self.calls) {
            self.registers[0x11] |= 0x80;
        }
        match operations {
            [Operation::Write(pointer), Operation::Read(data)] => {
                assert_eq!(pointer.len(), 1);
                assert_eq!(data.len(), 1);
                data[0] = self.registers[pointer[0] as usize];
            }
            [Operation::Write(data)] => {
                assert_eq!(data.len(), 2);
                self.writes.push((data[0], data[1]));
                if !self.ignore_write {
                    self.registers[data[0] as usize] = data[1];
                }
            }
            _ => panic!("unexpected transaction"),
        }
        Ok(())
    }
}

#[test]
fn input_or_boost_blocks_every_cut_without_configuration_writes() {
    for (register, bit) in [
        (0x11, 0x80),
        (0x0b, 4),
        (0x0b, 0x20),
        (0x0b, 0x40),
        (0x0b, 0xe0),
        (0x03, 0x20),
    ] {
        let mut bus = Bus::default();
        bus.registers[register] = bit;
        assert_eq!(ship::cut(&mut bus), Err(ship::Error::ExternalPower));
        assert!(bus.writes.is_empty());
    }
}

#[test]
fn cut_preserves_configuration_but_never_replays_one_shot_commands() {
    for value in 0..=255 {
        let mut bus = Bus::default();
        bus.registers[9] = value;
        ship::cut(&mut bus).unwrap();
        assert_eq!(bus.writes, [(9, (value & 0x54) | 0x20)]);
        ship::cancel(&mut bus).unwrap();
        assert_eq!(bus.registers[9], value & 0x54);
    }
}

#[test]
fn every_transport_failure_aborts_without_retrying_the_cut() {
    for step in 1..=5 {
        let mut bus = Bus {
            fail_at: Some(step),
            ..Bus::default()
        };
        assert_eq!(ship::cut(&mut bus), Err(ship::Error::Bus(ErrorKind::Other)));
        assert!(bus.writes.is_empty());
        assert_eq!(bus.calls, step);
    }
}

#[test]
fn input_is_checked_again_at_commit_even_after_successful_preflight() {
    let mut bus = Bus::default();
    ship::check(&mut bus).unwrap();
    bus.insert_at = Some(bus.calls + 2);
    assert_eq!(ship::cut(&mut bus), Err(ship::Error::ExternalPower));
    assert!(bus.writes.is_empty());
}

#[test]
fn acknowledged_cancel_is_not_enough_if_the_cut_bit_remains_set() {
    let mut bus = Bus {
        ignore_write: true,
        ..Bus::default()
    };
    bus.registers[9] = 0x20;
    assert_eq!(ship::cancel(&mut bus), Err(ship::Error::Verification));
    bus.ignore_write = false;
    ship::cancel(&mut bus).unwrap();
    assert_eq!(bus.registers[9] & 0x28, 0);
}

#[test]
fn cancellation_requires_all_three_bus_transactions() {
    for step in 1..=3 {
        let mut bus = Bus {
            fail_at: Some(step),
            ..Bus::default()
        };
        bus.registers[9] = 0x20;
        assert_eq!(
            ship::cancel(&mut bus),
            Err(ship::Error::Bus(ErrorKind::Other))
        );
        assert_eq!(bus.calls, step);
    }
}
