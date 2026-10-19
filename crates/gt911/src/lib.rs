//! GT911 single-contact reports through a borrowed I2C bus.
//! Register definitions follow Goodix Programming Guide Rev.10, sections 3.3 and 5.
//! The driver preserves the controller configuration.

#![no_std]

use embedded_hal::i2c::I2c;

pub const CONFIG_BYTES: usize = 186;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact {
    pub id: u8,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Down(Contact),
    Up(Contact),
    Cancel,
}

pub struct Gt911 {
    pub address: u8,
    pub width: u16,
    pub height: u16,
    pub firmware: u16,
    pub vendor: u8,
    contact: Option<Contact>,
    blocked: bool,
    key_state: Option<bool>,
}

impl Gt911 {
    /// Probe both documented addresses without a reset or configuration write.
    pub fn probe(bus: &mut impl I2c) -> Option<Self> {
        for address in [0x5d, 0x14] {
            let mut identity = [0; 11];
            if bus
                .write_read(address, &[0x81, 0x40], &mut identity)
                .is_ok()
                && identity[..4] == *b"911\0"
            {
                let width = u16::from_le_bytes([identity[6], identity[7]]);
                let height = u16::from_le_bytes([identity[8], identity[9]]);
                if width == 0 || height == 0 {
                    continue;
                }
                return Some(Self {
                    address,
                    width,
                    height,
                    firmware: u16::from_le_bytes([identity[4], identity[5]]),
                    vendor: identity[10],
                    contact: None,
                    blocked: false,
                    key_state: None,
                });
            }
        }
        None
    }

    pub fn read_config<B: I2c>(
        &self,
        bus: &mut B,
        output: &mut [u8; CONFIG_BYTES],
    ) -> Result<(), B::Error> {
        bus.write_read(self.address, &[0x80, 0x47], output)
    }

    pub fn cancel(&mut self) {
        self.contact = None;
        self.blocked = true;
    }

    /// Returns the key level from the last acknowledged ready report.
    pub fn take_key_state(&mut self) -> Option<bool> {
        self.key_state.take()
    }

    pub fn poll<B: I2c>(&mut self, bus: &mut B) -> Result<Option<Event>, B::Error> {
        let result = self.read_report(bus);
        if result.is_err() {
            self.cancel();
        }
        result
    }

    fn read_report<B: I2c>(&mut self, bus: &mut B) -> Result<Option<Event>, B::Error> {
        let mut status = [0];
        bus.write_read(self.address, &[0x81, 0x4e], &mut status)?;
        if status[0] & 0x80 == 0 {
            return Ok(None);
        }
        let count = status[0] & 15;
        let mut point = [0; 8];
        if count == 1 {
            bus.write_read(self.address, &[0x81, 0x4f], &mut point)?;
        }
        // Acknowledge only after all required data arrived.
        bus.write(self.address, &[0x81, 0x4e, 0])?;
        self.key_state = Some(status[0] & 0x10 != 0);
        if count == 0 {
            self.blocked = false;
            return Ok(self.contact.take().map(Event::Up));
        }
        let contact = Contact {
            id: point[0] & 15,
            x: u16::from_le_bytes([point[1], point[2]]),
            y: u16::from_le_bytes([point[3], point[4]]),
        };
        if count != 1
            || contact.x >= self.width
            || contact.y >= self.height
            || self
                .contact
                .is_some_and(|previous| previous.id != contact.id)
        {
            self.cancel();
            return Ok(Some(Event::Cancel));
        }
        if self.blocked {
            return Ok(None);
        }
        let first = self.contact.replace(contact).is_none();
        Ok(first.then_some(Event::Down(contact)))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};
    use std::{collections::VecDeque, vec, vec::Vec};

    enum Step {
        Read(u16, Vec<u8>),
        Ack,
        Fail(u16),
    }
    struct Bus(VecDeque<Step>);
    impl ErrorType for Bus {
        type Error = ErrorKind;
    }
    impl I2c for Bus {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), ErrorKind> {
            assert_eq!(address, 0x5d);
            match (
                self.0.pop_front().expect("unexpected bus operation"),
                operations,
            ) {
                (
                    Step::Read(register, data),
                    [Operation::Write(command), Operation::Read(output)],
                ) => {
                    assert_eq!(*command, register.to_be_bytes());
                    output.copy_from_slice(&data);
                    Ok(())
                }
                (Step::Fail(register), [Operation::Write(command), Operation::Read(_)]) => {
                    assert_eq!(*command, register.to_be_bytes());
                    Err(ErrorKind::Other)
                }
                (Step::Ack, [Operation::Write(command)]) => {
                    assert_eq!(*command, [0x81, 0x4e, 0]);
                    Ok(())
                }
                _ => panic!("incorrect operation order"),
            }
        }
    }
    fn controller() -> Gt911 {
        Gt911 {
            address: 0x5d,
            width: 540,
            height: 960,
            firmware: 0,
            vendor: 0,
            contact: None,
            blocked: false,
            key_state: None,
        }
    }
    fn point(x: u16, y: u16) -> Vec<u8> {
        vec![
            0,
            x as u8,
            (x >> 8) as u8,
            y as u8,
            (y >> 8) as u8,
            10,
            0,
            0,
        ]
    }
    #[test]
    fn key_levels_require_ready_acknowledged_reports() {
        let mut bus = Bus(VecDeque::from([
            Step::Read(0x814e, vec![0x10]),
            Step::Read(0x814e, vec![0x90]),
            Step::Ack,
            Step::Read(0x814e, vec![0x80]),
            Step::Ack,
        ]));
        let mut driver = controller();
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(driver.take_key_state(), None);
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(driver.take_key_state(), Some(true));
        assert_eq!(driver.take_key_state(), None);
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(driver.take_key_state(), Some(false));
        assert!(bus.0.is_empty());
    }

    #[test]
    fn ready_reports_create_one_gesture_and_keep_the_last_position() {
        let mut bus = Bus(VecDeque::from([
            Step::Read(0x814e, vec![0x81]),
            Step::Read(0x814f, point(200, 400)),
            Step::Ack,
            Step::Read(0x814e, vec![0x01]),
            Step::Read(0x814e, vec![0x81]),
            Step::Read(0x814f, point(210, 410)),
            Step::Ack,
            Step::Read(0x814e, vec![0x80]),
            Step::Ack,
            Step::Read(0x814e, vec![0x80]),
            Step::Ack,
        ]));
        let mut driver = controller();
        assert_eq!(
            driver.poll(&mut bus),
            Ok(Some(Event::Down(Contact {
                id: 0,
                x: 200,
                y: 400
            })))
        );
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(
            driver.poll(&mut bus),
            Ok(Some(Event::Up(Contact {
                id: 0,
                x: 210,
                y: 410
            })))
        );
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert!(bus.0.is_empty());
    }
    #[test]
    fn read_failure_does_not_acknowledge_or_complete_a_gesture() {
        let mut bus = Bus(VecDeque::from([
            Step::Read(0x814e, vec![0x81]),
            Step::Read(0x814f, point(200, 400)),
            Step::Ack,
            Step::Read(0x814e, vec![0x81]),
            Step::Fail(0x814f),
            Step::Read(0x814e, vec![0x80]),
            Step::Ack,
        ]));
        let mut driver = controller();
        assert!(driver.poll(&mut bus).unwrap().is_some());
        assert!(driver.poll(&mut bus).is_err());
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert!(bus.0.is_empty());
    }
    #[test]
    fn multiple_contacts_cancel_until_release() {
        let mut bus = Bus(VecDeque::from([
            Step::Read(0x814e, vec![0x81]),
            Step::Read(0x814f, point(200, 400)),
            Step::Ack,
            Step::Read(0x814e, vec![0x82]),
            Step::Ack,
            Step::Read(0x814e, vec![0x81]),
            Step::Read(0x814f, point(200, 400)),
            Step::Ack,
            Step::Read(0x814e, vec![0x80]),
            Step::Ack,
        ]));
        let mut driver = controller();
        assert!(driver.poll(&mut bus).unwrap().is_some());
        assert_eq!(driver.poll(&mut bus), Ok(Some(Event::Cancel)));
        assert_eq!(driver.poll(&mut bus), Ok(None));
        assert_eq!(driver.poll(&mut bus), Ok(None));
    }
}
