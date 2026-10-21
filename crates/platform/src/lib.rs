#![no_std]

//! Reusable application toolkit, service contracts, and coordinate adapters.
//! This crate has no application or backend dependency.
//!
//! The platform adapter controls SPI, I2C, GPIO, interrupts, and executor selection.
//! Drivers can use embedded-hal below these interfaces.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

pub mod discovery;
pub mod display;
pub mod input;
pub mod location;
pub mod power;
pub mod rotation;
pub mod suspend;
pub mod time;
pub mod touchpad;
pub mod ui;

/// Monotonic time and optional UTC calendar time.
pub trait Clock {
    /// Milliseconds from a stable epoch, independent of UTC corrections.
    fn now_ms(&self) -> u64;

    /// Current UTC, or None when calendar time is unknown or invalid.
    /// This read requires no I/O, wait, or allocation.
    fn now_utc(&self) -> Option<time::UtcDateTime> {
        None
    }
}

/// Coordinates use logical display orientation at the application boundary.
/// Platform input adapters can also use this type before coordinate normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact {
    /// Stable identity from 0 through 15 for one contact lifetime.
    pub id: u8,
    pub position: Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchEvent {
    /// A new or updated contact. Repeated reports keep the same identity.
    Touch(Contact),
    Release(Contact),
    Cancel,
}

impl TouchEvent {
    pub const fn touch(id: u8, position: Point) -> Self {
        Self::Touch(Contact { id, position })
    }
    pub const fn release(id: u8, position: Point) -> Self {
        Self::Release(Contact { id, position })
    }

    /// Preserves identity during coordinate conversion. Invalid coordinates cancel input.
    pub fn map_position(self, convert: impl FnOnce(Point) -> Option<Point>) -> Self {
        match self {
            Self::Touch(contact) => convert(contact.position)
                .map(|p| Self::touch(contact.id, p))
                .unwrap_or(Self::Cancel),
            Self::Release(contact) => convert(contact.position)
                .map(|p| Self::release(contact.id, p))
                .unwrap_or(Self::Cancel),
            Self::Cancel => Self::Cancel,
        }
    }
}

/// Drawing updates a staging buffer without a visible panel change.
/// A successful refresh submits that buffer.
/// Submission does not confirm panel completion.
///
/// While busy, the display permits no drawing or new refresh.
/// The adapter controls buffers, completion detection, refresh, and power sequences.
/// After an error, the application keeps its redraw request and can retry.
pub trait Display: DrawTarget<Color = Gray4> {
    fn is_busy(&self) -> bool;
    fn refresh(&mut self) -> Result<(), Self::Error>;
}
