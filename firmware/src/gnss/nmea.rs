//! Bounded NMEA envelopes shared by startup probing and observation decoding.
//!
//! See [GNSS input](../../../docs/boards/lilygo-t5-s3-pro-gnss.md).

/// One partial NMEA sentence, discarded at each receive-session boundary.
pub struct Sentence {
    frame: [u8; 128],
    length: usize,
    collecting: bool,
}

impl Default for Sentence {
    fn default() -> Self {
        Self {
            frame: [0; 128],
            length: 0,
            collecting: false,
        }
    }
}

impl Sentence {
    pub fn clear(&mut self) {
        self.length = 0;
        self.collecting = false;
    }

    pub fn feed(&mut self, byte: u8) -> Option<&str> {
        if byte == b'$' {
            self.length = 0;
            self.collecting = true;
        } else if self.collecting && byte == b'\n' {
            self.collecting = false;
            return message(&self.frame[..self.length]);
        } else if self.collecting {
            if self.length == self.frame.len() {
                self.collecting = false;
            } else {
                self.frame[self.length] = byte;
                self.length += 1;
            }
        }
        None
    }
}

fn message(frame: &[u8]) -> Option<&str> {
    let frame = frame.strip_suffix(b"\r").unwrap_or(frame);
    let star = frame.iter().position(|byte| *byte == b'*')?;
    if star + 3 != frame.len() {
        return None;
    }
    let checksum = hex(frame[star + 1])? * 16 + hex(frame[star + 2])?;
    if frame[..star].iter().fold(0, |sum, byte| sum ^ byte) != checksum {
        return None;
    }
    let text = core::str::from_utf8(&frame[..star]).ok()?;
    let header = text.split(',').next()?;
    if header.len() != 5
        || !header.bytes().all(|byte| byte.is_ascii_uppercase())
        || !matches!(
            &header[..2],
            "GP" | "GN" | "GL" | "GA" | "GB" | "BD" | "GQ" | "GI"
        )
        || !text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
        || !text.contains(',')
    {
        return None;
    }
    Some(text)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
