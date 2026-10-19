//! GT911 contact frames through a borrowed I2C bus.
//! Goodix Programming Guide Rev.10, sections 3.3 and 5, defines the report sequence.

#![no_std]

use embedded_hal::i2c::I2c;

pub const CONFIG_BYTES: usize = 186;
pub const MAX_CONTACTS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact {
    pub id: u8,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Touch(Contact),
    Release(Contact),
    Cancel,
}

/// All changes from one acknowledged controller report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    events: [Option<Event>; MAX_CONTACTS * 2],
    len: usize,
}

impl Frame {
    fn new() -> Self {
        Self {
            events: [None; MAX_CONTACTS * 2],
            len: 0,
        }
    }
    pub fn canceled() -> Self {
        let mut frame = Self::new();
        frame.push(Event::Cancel);
        frame
    }
    fn push(&mut self, event: Event) {
        self.events[self.len] = Some(event);
        self.len += 1;
    }
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events[..self.len].iter().flatten().copied()
    }
}

pub struct Gt911 {
    pub address: u8,
    pub width: u16,
    pub height: u16,
    pub firmware: u16,
    pub vendor: u8,
    contacts: [Option<Contact>; MAX_CONTACTS],
    blocked: bool,
    key_state: Option<bool>,
}

impl Gt911 {
    /// Probe both addresses without a reset or configuration write.
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
                    contacts: [None; MAX_CONTACTS],
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
        self.contacts = [None; MAX_CONTACTS];
        self.blocked = true;
        self.key_state = None;
    }

    /// Returns the key level from the last acknowledged ready report.
    pub fn take_key_state(&mut self) -> Option<bool> {
        self.key_state.take()
    }

    pub fn poll<B: I2c>(&mut self, bus: &mut B) -> Result<Option<Frame>, B::Error> {
        let result = self.read_report(bus);
        if result.is_err() {
            self.cancel();
        }
        result
    }

    fn read_report<B: I2c>(&mut self, bus: &mut B) -> Result<Option<Frame>, B::Error> {
        let mut status = [0];
        bus.write_read(self.address, &[0x81, 0x4e], &mut status)?;
        if status[0] & 0x80 == 0 {
            return Ok(None);
        }
        let count = usize::from(status[0] & 15);
        let mut bytes = [0; MAX_CONTACTS * 8];
        if count > 0 && count <= MAX_CONTACTS {
            bus.write_read(self.address, &[0x81, 0x4f], &mut bytes[..count * 8])?;
        }
        // A failed read or acknowledgement never publishes a partial frame.
        bus.write(self.address, &[0x81, 0x4e, 0])?;
        self.key_state = Some(status[0] & 0x10 != 0);
        if self.blocked {
            if count == 0 {
                self.blocked = false;
            }
            return Ok(None);
        }
        let mut next = [None; MAX_CONTACTS];
        let mut ids = 0_u16;
        let mut invalid = count > MAX_CONTACTS || status[0] & 0x40 != 0;
        for (slot, bytes) in next.iter_mut().zip(bytes.chunks_exact(8)).take(count) {
            let contact = Contact {
                id: bytes[0],
                x: u16::from_le_bytes([bytes[1], bytes[2]]),
                y: u16::from_le_bytes([bytes[3], bytes[4]]),
            };
            let bit = 1_u16.checked_shl(u32::from(contact.id)).unwrap_or(0);
            invalid |=
                bit == 0 || ids & bit != 0 || contact.x >= self.width || contact.y >= self.height;
            ids |= bit;
            *slot = Some(contact);
        }
        if invalid {
            self.contacts = [None; MAX_CONTACTS];
            self.blocked = count != 0;
            return Ok(Some(Frame::canceled()));
        }
        let mut frame = Frame::new();
        for old in self.contacts.iter().flatten() {
            if !next.iter().flatten().any(|c| c.id == old.id) {
                frame.push(Event::Release(*old));
            }
        }
        for contact in next.iter().flatten() {
            frame.push(Event::Touch(*contact));
        }
        self.contacts = next;
        Ok((frame.len > 0).then_some(frame))
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
        AckFail,
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
                (Step::AckFail, [Operation::Write(command)]) => {
                    assert_eq!(*command, [0x81, 0x4e, 0]);
                    Err(ErrorKind::Other)
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
            contacts: [None; MAX_CONTACTS],
            blocked: false,
            key_state: None,
        }
    }
    fn point(id: u8, x: u16, y: u16) -> Contact {
        Contact { id, x, y }
    }
    fn report(bus: &mut Bus, contacts: &[Contact]) {
        bus.0
            .push_back(Step::Read(0x814e, vec![0x80 | contacts.len() as u8]));
        if !contacts.is_empty() {
            let bytes = contacts
                .iter()
                .flat_map(|c| {
                    [
                        c.id,
                        c.x as u8,
                        (c.x >> 8) as u8,
                        c.y as u8,
                        (c.y >> 8) as u8,
                        10,
                        0,
                        0,
                    ]
                })
                .collect();
            bus.0.push_back(Step::Read(0x814f, bytes));
        }
        bus.0.push_back(Step::Ack);
    }
    fn events(driver: &mut Gt911, bus: &mut Bus) -> Vec<Event> {
        driver
            .poll(bus)
            .unwrap()
            .map(|f| f.events().collect())
            .unwrap_or_default()
    }

    #[test]
    fn reordering_movement_and_partial_release_preserve_ids() {
        let mut bus = Bus(VecDeque::new());
        let mut driver = controller();
        let a = point(3, 100, 200);
        let b = point(12, 300, 400);
        report(&mut bus, &[a, b]);
        assert_eq!(
            events(&mut driver, &mut bus),
            vec![Event::Touch(a), Event::Touch(b)]
        );
        let moved = point(3, 110, 220);
        report(&mut bus, &[b, moved]);
        assert_eq!(
            events(&mut driver, &mut bus),
            vec![Event::Touch(b), Event::Touch(moved)]
        );
        report(&mut bus, &[b]);
        assert_eq!(
            events(&mut driver, &mut bus),
            vec![Event::Release(moved), Event::Touch(b)]
        );
        report(&mut bus, &[]);
        assert_eq!(events(&mut driver, &mut bus), vec![Event::Release(b)]);
        assert!(bus.0.is_empty());
    }

    #[test]
    fn five_contacts_can_all_change_identity_in_one_report() {
        let mut driver = controller();
        let mut bus = Bus(VecDeque::new());
        let a = core::array::from_fn::<_, 5, _>(|i| point(i as u8, 100, 200));
        let b = core::array::from_fn::<_, 5, _>(|i| point(i as u8 + 5, 300, 400));
        report(&mut bus, &a);
        events(&mut driver, &mut bus);
        report(&mut bus, &b);
        let result = events(&mut driver, &mut bus);
        assert_eq!(result.len(), 10);
        assert_eq!(&result[..5], &a.map(Event::Release));
        assert_eq!(&result[5..], &b.map(Event::Touch));
    }

    #[test]
    fn malformed_reports_cancel_until_all_contacts_lift() {
        for contacts in [
            vec![point(1, 540, 0)],
            vec![point(2, 0, 960)],
            vec![point(32, 0, 0)],
            vec![point(1, 0, 0), point(1, 2, 3)],
        ] {
            let mut driver = controller();
            let mut bus = Bus(VecDeque::new());
            report(&mut bus, &contacts);
            assert_eq!(events(&mut driver, &mut bus), vec![Event::Cancel]);
            report(&mut bus, &[point(0, 1, 2)]);
            assert!(events(&mut driver, &mut bus).is_empty());
            report(&mut bus, &[]);
            events(&mut driver, &mut bus);
            report(&mut bus, &[point(0, 1, 2)]);
            assert_eq!(
                events(&mut driver, &mut bus),
                vec![Event::Touch(point(0, 1, 2))]
            );
        }
        let mut driver = controller();
        let mut bus = Bus(VecDeque::from([Step::Read(0x814e, vec![0x86]), Step::Ack]));
        assert_eq!(events(&mut driver, &mut bus), vec![Event::Cancel]);
    }

    #[test]
    fn failures_publish_nothing_and_do_not_acknowledge_incomplete_reads() {
        for fail_ack in [false, true] {
            let mut driver = controller();
            let mut bus = Bus(VecDeque::new());
            report(&mut bus, &[point(1, 2, 3)]);
            events(&mut driver, &mut bus);
            if fail_ack {
                report(&mut bus, &[]);
                bus.0.pop_back();
                bus.0.push_back(Step::AckFail);
            } else {
                bus.0
                    .extend([Step::Read(0x814e, vec![0x82]), Step::Fail(0x814f)]);
            }
            assert!(driver.poll(&mut bus).is_err());
            assert_eq!(driver.take_key_state(), None);
            assert!(bus.0.is_empty());
            report(&mut bus, &[]);
            assert!(events(&mut driver, &mut bus).is_empty());
        }
    }

    #[test]
    fn key_and_palm_bits_require_ready_acknowledged_reports() {
        let mut driver = controller();
        let mut bus = Bus(VecDeque::from([
            Step::Read(0x814e, vec![0x10]),
            Step::Read(0x814e, vec![0x90]),
            Step::Ack,
            Step::Read(0x814e, vec![0xc0]),
            Step::Ack,
        ]));
        assert!(events(&mut driver, &mut bus).is_empty());
        assert_eq!(driver.take_key_state(), None);
        assert!(events(&mut driver, &mut bus).is_empty());
        assert_eq!(driver.take_key_state(), Some(true));
        assert_eq!(events(&mut driver, &mut bus), vec![Event::Cancel]);
        assert_eq!(driver.take_key_state(), Some(false));
    }
}
