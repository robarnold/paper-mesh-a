//! Charger isolation and startup recovery tests.
//!
//! # Design choices
//!
//! - Inject supply changes and bus failures around battery isolation so uncertainty
//!   cannot trigger a second cut request.
//!
//! See [System power-off](../../docs/boards/lilygo-t5-s3-pro-power.md#system-power-off).

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

// External input or boost must refuse isolation before any configuration write.
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

// Isolation and startup must preserve settings without replaying command bits.
#[test]
fn cut_preserves_configuration_but_never_replays_one_shot_commands() {
    for value in 0..=255 {
        let mut bus = Bus::default();
        bus.registers[9] = value;
        ship::cut(&mut bus).unwrap();
        assert_eq!(bus.writes, [(9, (value & 0x54) | 0x20)]);
        ship::initialize(&mut bus).unwrap();
        assert_eq!(bus.registers[9], value & 0x54);
    }
}

// Each access failure must stop the cut sequence without retrying a possible write.
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

// A newly attached supply must refuse the cut even after successful preflight.
#[test]
fn input_is_checked_again_at_commit_even_after_successful_preflight() {
    let mut bus = Bus::default();
    ship::check(&mut bus).unwrap();
    bus.insert_at = Some(bus.calls + 2);
    assert_eq!(ship::cut(&mut bus), Err(ship::Error::ExternalPower));
    assert!(bus.writes.is_empty());
}

// Startup must reject retained isolation when a write succeeds without changing the register.
#[test]
fn startup_rejects_an_acknowledged_write_if_the_cut_bit_remains_set() {
    let mut bus = Bus {
        ignore_write: true,
        ..Bus::default()
    };
    bus.registers[9] = 0x20;
    assert_eq!(ship::initialize(&mut bus), Err(ship::Error::Verification));
    bus.ignore_write = false;
    ship::initialize(&mut bus).unwrap();
    assert_eq!(bus.registers[9] & 0x28, 0);
}

// Failed startup reads, writes, or verification must prevent a ready result.
#[test]
fn startup_requires_all_three_bus_transactions() {
    for step in 1..=3 {
        let mut bus = Bus {
            fail_at: Some(step),
            ..Bus::default()
        };
        bus.registers[9] = 0x20;
        assert_eq!(
            ship::initialize(&mut bus),
            Err(ship::Error::Bus(ErrorKind::Other))
        );
        assert_eq!(bus.calls, step);
    }
}

// Cold start and repeated reset recovery must converge on the same retained configuration.
#[test]
fn startup_clears_retained_commands_and_is_repeatable() {
    for value in 0..=255 {
        let mut bus = Bus::default();
        bus.registers[9] = value;
        ship::initialize(&mut bus).unwrap();
        assert_eq!(bus.registers[9], value & 0x54);
        ship::initialize(&mut bus).unwrap();
        assert_eq!(bus.registers[9], value & 0x54);
        assert_eq!(bus.writes, [(9, value & 0x54), (9, value & 0x54)]);
    }
}
