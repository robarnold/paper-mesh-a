#![no_std]

//! USB HID descriptors and complete contact reports for a five-contact touchpad.

use paper_mesh_platform::{
    Contact,
    input::contacts::{ContactState, MAX_CONTACTS},
    touchpad::HostFrame,
};

pub const REPORT_LEN: usize = 34;
// Linux stores an integer resolution per millimeter. Scale wire coordinates to limit rounding.
pub const COORDINATE_SCALE: u16 = 16;
pub const INPUT_REPORT: u8 = 1;
pub const MAXIMUM_REPORT: u8 = 2;
pub const CERTIFICATION_REPORT: u8 = 3;
pub const MODE_REPORT: u8 = 4;

/// Logical coordinates and active surface dimensions in tenths of a millimeter.
pub struct Geometry {
    pub width: u16,
    pub height: u16,
    pub width_tenth_mm: u16,
    pub height_tenth_mm: u16,
}

pub struct Descriptor {
    bytes: [u8; 512],
    len: usize,
}
impl Descriptor {
    const fn append(&mut self, data: &[u8]) {
        let mut i = 0;
        while i < data.len() {
            self.bytes[self.len] = data[i];
            self.len += 1;
            i += 1;
        }
    }
    pub const fn new(g: Geometry) -> Self {
        assert!(g.width > 1 && g.height > 1 && g.width_tenth_mm > 0 && g.height_tenth_mm > 0);
        assert!(g.width <= 4096 && g.height <= 4096);
        let x_max = (g.width - 1) * COORDINATE_SCALE;
        let y_max = (g.height - 1) * COORDINATE_SCALE;
        let mut d = Self {
            bytes: [0; 512],
            len: 0,
        };
        // Digitizers / Touch Pad / Application / Report ID.
        d.append(&[0x05, 0x0d, 0x09, 0x05, 0xa1, 0x01, 0x85, INPUT_REPORT]);
        let mut finger = 0;
        while finger < MAX_CONTACTS {
            // Finger logical collection. Tip Switch and Confidence, then padding.
            d.append(&[
                0x05, 0x0d, 0x09, 0x22, 0xa1, 0x02, 0x55, 0x00, 0x65, 0x00, 0x15, 0x00, 0x25, 0x01,
                0x35, 0x00, 0x45, 0x00, 0x09, 0x42, 0x09, 0x47, 0x75, 0x01, 0x95, 0x02, 0x81, 0x02,
                0x75, 0x06, 0x95, 0x01, 0x81, 0x03, 0x09, 0x51, 0x25, 0x0f, 0x75, 0x08, 0x81, 0x02,
            ]);
            // X and Y use SI centimeters with exponent -2: 0.1 mm per physical unit.
            d.append(&[
                0x05,
                0x01,
                0x55,
                0x0e,
                0x65,
                0x11,
                0x75,
                0x10,
                0x09,
                0x30,
                0x26,
                x_max as u8,
                (x_max >> 8) as u8,
                0x46,
                g.width_tenth_mm as u8,
                (g.width_tenth_mm >> 8) as u8,
                0x81,
                0x02,
                0x09,
                0x31,
                0x26,
                y_max as u8,
                (y_max >> 8) as u8,
                0x46,
                g.height_tenth_mm as u8,
                (g.height_tenth_mm >> 8) as u8,
                0x81,
                0x02,
                0xc0,
            ]);
            finger += 1;
        }
        // Contact Count includes release entries. Scan Time uses units of 100 us.
        d.append(&[
            0x05,
            0x0d,
            0x55,
            0x00,
            0x65,
            0x00,
            0x35,
            0x00,
            0x45,
            0x00,
            0x09,
            0x54,
            0x25,
            0x05,
            0x75,
            0x08,
            0x95,
            0x01,
            0x81,
            0x02,
            0x09,
            0x56,
            0x27,
            0xff,
            0xff,
            0x00,
            0x00,
            0x75,
            0x10,
            0x55,
            0x0c,
            0x66,
            0x01,
            0x10,
            0x81,
            0x02,
            0x85,
            MAXIMUM_REPORT,
            0x09,
            0x55,
            0x55,
            0x00,
            0x65,
            0x00,
            0x25,
            0x05,
            0x75,
            0x08,
            0xb1,
            0x02,
            // This feature selects Linux's generic Win8 multitouch class.
            // Its zero payload does not claim Windows certification.
            0x85,
            CERTIFICATION_REPORT,
            0x06,
            0x00,
            0xff,
            0x09,
            0xc5,
            0x26,
            0xff,
            0x00,
            0x96,
            0x00,
            0x01,
            0xb1,
            0x02,
            0xc0,
            // Digitizer Configuration / Input Mode (3 = native touchpad).
            0x05,
            0x0d,
            0x09,
            0x0e,
            0xa1,
            0x01,
            0x85,
            MODE_REPORT,
            0x09,
            0x52,
            0x25,
            0x03,
            0x95,
            0x01,
            0xb1,
            0x02,
            0xc0,
        ]);
        d
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Supplies HID feature payloads, including the report ID byte.
pub fn get_feature(id: u8, out: &mut [u8]) -> Option<usize> {
    let len = match id {
        MAXIMUM_REPORT | MODE_REPORT => 2,
        CERTIFICATION_REPORT => 257,
        _ => return None,
    };
    if out.len() < len {
        return None;
    }
    out[..len].fill(0);
    out[0] = id;
    if id == MAXIMUM_REPORT {
        out[1] = MAX_CONTACTS as u8;
    }
    if id == MODE_REPORT {
        out[1] = 3;
    }
    Some(len)
}

pub fn set_feature(id: u8, data: &[u8]) -> bool {
    id == MODE_REPORT && data == [MODE_REPORT, 3]
}

pub struct Reports {
    bytes: [[u8; REPORT_LEN]; 2],
    len: usize,
}
impl Reports {
    pub fn packets(&self) -> impl Iterator<Item = &[u8; REPORT_LEN]> {
        self.bytes[..self.len].iter()
    }
}

#[derive(Default)]
pub struct Encoder {
    previous: ContactState,
    last_scan: Option<u64>,
}
impl Encoder {
    /// Advances the delivered state. Reset the encoder if transport fails.
    pub fn encode(&mut self, frame: HostFrame) -> Reports {
        let mut entries: [(Option<Contact>, bool); 10] = [(None, false); 10];
        let mut len = 0;
        for old in self.previous.contacts() {
            if !frame
                .contacts
                .contacts()
                .any(|contact| contact.id == old.id)
            {
                entries[len] = (Some(old), false);
                len += 1;
            }
        }
        let released = len;
        for contact in frame.contacts.contacts() {
            entries[len] = (Some(contact), true);
            len += 1;
        }
        let mut reports = Reports {
            bytes: [[0; REPORT_LEN]; 2],
            len: 1,
        };
        let mut time = frame.at_ms.saturating_mul(10);
        if let Some(previous) = self.last_scan {
            time = time.max(previous.saturating_add(1));
        }
        if len > MAX_CONTACTS {
            // Complete the old frame before replacement IDs occupy its slots.
            let mut old_entries = [(None, false); 10];
            let mut old_len = 0;
            for old in self.previous.contacts() {
                let current = frame
                    .contacts
                    .contacts()
                    .find(|contact| contact.id == old.id);
                old_entries[old_len] = (Some(current.unwrap_or(old)), current.is_some());
                old_len += 1;
            }
            reports.bytes[0] = packet(&old_entries[..old_len], time as u16);
            time = time.saturating_add(1);
            reports.bytes[1] = packet(&entries[released..len], time as u16);
            reports.len = 2;
        } else {
            reports.bytes[0] = packet(&entries[..len], time as u16);
        }
        self.previous = frame.contacts;
        self.last_scan = Some(time);
        reports
    }
}

fn packet(entries: &[(Option<Contact>, bool)], time: u16) -> [u8; REPORT_LEN] {
    let mut data = [0; REPORT_LEN];
    data[0] = INPUT_REPORT;
    for (index, (contact, tip)) in entries.iter().enumerate() {
        if let Some(contact) = contact {
            let offset = 1 + index * 6;
            data[offset] = 2 | u8::from(*tip);
            data[offset + 1] = contact.id;
            data[offset + 2..offset + 4]
                .copy_from_slice(&((contact.position.x as u16) * COORDINATE_SCALE).to_le_bytes());
            data[offset + 4..offset + 6]
                .copy_from_slice(&((contact.position.y as u16) * COORDINATE_SCALE).to_le_bytes());
        }
    }
    data[31] = entries.len() as u8;
    data[32..34].copy_from_slice(&time.to_le_bytes());
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::Point;
    use paper_mesh_platform::{TouchEvent, input::contacts::TouchFrame};
    fn state(ids: &[u8]) -> ContactState {
        let mut state = ContactState::new();
        let mut frame = TouchFrame::new(0);
        for id in ids {
            frame.push(TouchEvent::touch(
                *id,
                Point::new(100 + i32::from(*id), 300),
            ));
        }
        assert!(state.apply(&frame));
        state
    }
    #[test]
    fn releases_keep_last_position_and_replacement_ids_get_complete_frames() {
        let mut encoder = Encoder::default();
        let first = encoder.encode(HostFrame {
            contacts: state(&[0, 1, 2, 3, 4]),
            at_ms: 10,
        });
        assert_eq!(first.bytes[0][31], 5);
        assert_eq!(&first.bytes[0][1..7], &[3, 0, 64, 6, 192, 18]);
        let next = encoder.encode(HostFrame {
            contacts: state(&[5, 6, 7, 8, 9]),
            at_ms: 11,
        });
        assert_eq!(next.len, 2);
        assert_eq!(&next.bytes[0][1..7], &[2, 0, 64, 6, 192, 18]);
        assert_eq!(&next.bytes[1][1..7], &[3, 5, 144, 6, 192, 18]);
        assert_eq!(next.bytes[1][32], 111);
        let end = encoder.encode(HostFrame {
            contacts: state(&[]),
            at_ms: 12,
        });
        assert_eq!(end.bytes[0][31], 5);
        assert_eq!(end.bytes[0][1], 2);
        assert_eq!(end.bytes[0][2], 5);
    }
    #[test]
    fn scan_time_advances_within_a_millisecond_and_wraps_on_the_wire() {
        let mut encoder = Encoder::default();
        let mut times = [0; 4];
        for time in &mut times {
            let reports = encoder.encode(HostFrame {
                contacts: state(&[1]),
                at_ms: 6553,
            });
            *time = u16::from_le_bytes([reports.bytes[0][32], reports.bytes[0][33]]);
        }
        assert_eq!(times, [65530, 65531, 65532, 65533]);
        let reports = encoder.encode(HostFrame {
            contacts: state(&[]),
            at_ms: 6554,
        });
        assert_eq!(&reports.bytes[0][32..], &[4, 0]);
    }
    #[test]
    fn feature_reports_reject_unknown_modes_and_short_buffers() {
        let mut bytes = [0xff; 257];
        assert_eq!(get_feature(MAXIMUM_REPORT, &mut bytes), Some(2));
        assert_eq!(&bytes[..2], &[MAXIMUM_REPORT, 5]);
        assert_eq!(get_feature(CERTIFICATION_REPORT, &mut bytes[..256]), None);
        assert_eq!(get_feature(CERTIFICATION_REPORT, &mut bytes), Some(257));
        assert!(bytes[1..].iter().all(|byte| *byte == 0));
        assert!(set_feature(MODE_REPORT, &[MODE_REPORT, 3]));
        assert!(!set_feature(MODE_REPORT, &[MODE_REPORT, 0]));
    }
}
