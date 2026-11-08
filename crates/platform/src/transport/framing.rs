//! Bounded newline frames independent of transport and command encoding.
//!
//! # Design choices
//!
//! - Let callers select capacity so protocol limits remain outside the shared mechanism.
//! - Discard overflow through the next newline so a suffix cannot become a command.
//!
//! See [request framing](../../../../docs/services/connectivity.md#request-framing).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    TooLong,
}

/// Collect bytes until LF, excluding LF from the returned frame.
/// Other bytes, including CR, remain unchanged. Empty frames are permitted.
#[derive(Default)]
pub struct LineFramer<const N: usize> {
    bytes: heapless::Vec<u8, N>,
    overflow: bool,
}

impl<const N: usize> LineFramer<N> {
    /// Discard partial input and overflow state when the source session changes.
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.overflow = false;
    }

    /// Return a complete frame or overflow error only at its terminating LF.
    /// After either result, the next byte starts a new frame.
    pub fn push(&mut self, byte: u8) -> Option<Result<heapless::Vec<u8, N>, FrameError>> {
        if byte == b'\n' {
            let result = if self.overflow {
                Err(FrameError::TooLong)
            } else {
                Ok(core::mem::take(&mut self.bytes))
            };
            self.clear();
            return Some(result);
        }
        if !self.overflow && self.bytes.push(byte).is_err() {
            self.overflow = true;
        }
        None
    }
}

#[cfg(test)]
mod tests;
