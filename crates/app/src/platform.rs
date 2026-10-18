//! Application services above individual chip drivers.
//!
//! The platform adapter controls SPI, I2C, GPIO, interrupts, and executor selection.
//! Drivers can use embedded-hal below these interfaces.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

pub mod discovery;
pub mod location;
pub mod rotation;

/// Native panel dimensions, independent of device pose.
pub const NATIVE_SCREEN_SIZE: Size = Size::new(960, 540);
/// Logical portrait dimensions. Adapters rotate panel and touch coordinates.
pub const SCREEN_SIZE: Size = Size::new(NATIVE_SCREEN_SIZE.height, NATIVE_SCREEN_SIZE.width);

/// Monotonic milliseconds from an arbitrary, stable epoch.
/// Wall-clock time and CPU frequency changes must not affect this clock.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

/// Coordinates use logical display orientation at the application boundary.
/// Platform input adapters can also use this type before coordinate normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchEvent {
    Down(Point),
    Up(Point),
    Cancel,
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
