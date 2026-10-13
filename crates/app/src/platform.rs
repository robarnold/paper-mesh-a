//! Services consumed by the application, above individual chip drivers.
//!
//! SPI, I2C, GPIO, interrupts, and executor selection belong in the platform
//! adapter. Drivers can use embedded-hal underneath these interfaces.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

pub mod rotation;

/// Panel storage/scan orientation, independent of how the device is held.
pub const NATIVE_SCREEN_SIZE: Size = Size::new(960, 540);
/// Logical portrait orientation; adapters rotate panel and touch coordinates.
pub const SCREEN_SIZE: Size = Size::new(NATIVE_SCREEN_SIZE.height, NATIVE_SCREEN_SIZE.width);

/// Monotonic milliseconds from an arbitrary, stable epoch. Wall-clock time and
/// CPU frequency changes must not affect this clock.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

/// At the application boundary, coordinates are in logical display orientation.
/// Platform input adapters can also use this type before normalizing coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchEvent {
    Down(Point),
    Up(Point),
    Cancel,
}

/// Drawing updates a staging buffer, not the visible panel. A successful
/// refresh submits that buffer; it does not mean the panel has finished.
///
/// While busy, neither drawing nor starting another refresh is permitted.
/// The adapter owns buffers, completion detection, and refresh/power sequencing.
/// On error the application retains its redraw request and may retry.
pub trait Display: DrawTarget<Color = Gray4> {
    fn is_busy(&self) -> bool;
    fn refresh(&mut self) -> Result<(), Self::Error>;
}
