//! A clockwise portrait view of a native display.
//! The adapter transforms pixel coordinates without a second framebuffer.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

use super::{Display, TouchEvent};

#[derive(Clone, Copy)]
pub struct PortraitRotation {
    native_size: Size,
}

impl PortraitRotation {
    pub const fn new(native_size: Size) -> Self {
        assert!(native_size.width > 0 && native_size.width <= i32::MAX as u32);
        assert!(native_size.height > 0 && native_size.height <= i32::MAX as u32);
        Self { native_size }
    }

    pub const fn size(self) -> Size {
        Size::new(self.native_size.height, self.native_size.width)
    }

    /// Portrait (x, y) -> native (y, native_height - 1 - x).
    pub fn to_native(self, point: Point) -> Option<Point> {
        contains(self.size(), point)
            .then(|| Point::new(point.y, self.native_size.height as i32 - 1 - point.x))
    }

    /// Native (x, y) -> portrait (native_height - 1 - y, x).
    pub fn to_portrait(self, point: Point) -> Option<Point> {
        contains(self.native_size, point)
            .then(|| Point::new(self.native_size.height as i32 - 1 - point.y, point.x))
    }

    /// Normalizes touch input aligned with the panel.
    /// Invalid contacts cancel a gesture.
    /// Controller calibration, scaling, and axis alignment must occur before this operation.
    pub fn touch_to_portrait(self, event: TouchEvent) -> TouchEvent {
        event.map_position(|point| self.to_portrait(point))
    }
}

fn contains(size: Size, point: Point) -> bool {
    point.x >= 0 && point.y >= 0 && point.x < size.width as i32 && point.y < size.height as i32
}

/// A borrowed native display that provides a portrait draw target.
/// The native display keeps ownership of refresh state, errors, and buffers.
pub struct PortraitDisplay<'a, D> {
    native: &'a mut D,
    rotation: PortraitRotation,
}

impl<'a, D: OriginDimensions> PortraitDisplay<'a, D> {
    pub fn new(native: &'a mut D) -> Self {
        let rotation = PortraitRotation::new(native.size());
        Self { native, rotation }
    }
}

impl<D> OriginDimensions for PortraitDisplay<'_, D> {
    fn size(&self) -> Size {
        self.rotation.size()
    }
}

impl<D: DrawTarget<Color = Gray4>> DrawTarget for PortraitDisplay<'_, D> {
    type Color = Gray4;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let rotation = self.rotation;
        self.native
            .draw_iter(pixels.into_iter().filter_map(|Pixel(point, color)| {
                rotation.to_native(point).map(|point| Pixel(point, color))
            }))
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.native.clear(color)
    }
}

impl<D: Display> Display for PortraitDisplay<'_, D> {
    fn is_busy(&self) -> bool {
        self.native.is_busy()
    }

    fn refresh(&mut self) -> Result<(), Self::Error> {
        self.native.refresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::Rectangle;

    #[derive(Default)]
    struct Panel {
        pixels: [u8; 12],
        busy: bool,
    }

    impl OriginDimensions for Panel {
        fn size(&self) -> Size {
            Size::new(4, 3)
        }
    }

    impl DrawTarget for Panel {
        type Color = Gray4;
        type Error = &'static str;

        fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
            &mut self,
            pixels: I,
        ) -> Result<(), Self::Error> {
            if self.busy {
                return Err("busy");
            }
            for Pixel(point, color) in pixels {
                assert!(self.bounding_box().contains(point));
                self.pixels[point.y as usize * 4 + point.x as usize] = color.luma();
            }
            Ok(())
        }
    }

    impl Display for Panel {
        fn is_busy(&self) -> bool {
            self.busy
        }

        fn refresh(&mut self) -> Result<(), Self::Error> {
            if self.busy {
                return Err("busy");
            }
            self.busy = true;
            Ok(())
        }
    }

    #[test]
    fn asymmetric_pattern_is_stored_in_native_row_order() {
        let mut native = Panel::default();
        let mut portrait = PortraitDisplay::new(&mut native);
        assert_eq!(portrait.size(), Size::new(3, 4));
        portrait
            .fill_contiguous(&portrait.bounding_box(), (1..=12).map(Gray4::new))
            .unwrap();
        assert_eq!(native.pixels, [3, 6, 9, 12, 2, 5, 8, 11, 1, 4, 7, 10]);
    }

    #[test]
    fn clipping_preserves_color_positions_in_contiguous_fills() {
        let mut native = Panel::default();
        let mut portrait = PortraitDisplay::new(&mut native);
        portrait
            .fill_contiguous(
                &Rectangle::new(Point::new(-1, -1), Size::new(5, 6)),
                (0..30).map(|value| Gray4::new(value % 16)),
            )
            .unwrap();
        assert_eq!(native.pixels, [8, 13, 2, 7, 7, 12, 1, 6, 6, 11, 0, 5]);
    }

    #[test]
    fn native_corners_and_edges_map_to_portrait_without_mirroring() {
        let rotation = PortraitRotation::new(Size::new(960, 540));
        for (native, portrait) in [
            (Point::new(0, 0), Point::new(539, 0)),
            (Point::new(959, 0), Point::new(539, 959)),
            (Point::new(0, 539), Point::new(0, 0)),
            (Point::new(959, 539), Point::new(0, 959)),
            (Point::new(123, 456), Point::new(83, 123)),
        ] {
            assert_eq!(rotation.to_portrait(native), Some(portrait));
            assert_eq!(rotation.to_native(portrait), Some(native));
            assert_eq!(
                rotation.touch_to_portrait(TouchEvent::touch(0, native)),
                TouchEvent::touch(0, portrait)
            );
            assert_eq!(
                rotation.touch_to_portrait(TouchEvent::release(0, native)),
                TouchEvent::release(0, portrait)
            );
        }
        for point in [
            Point::new(-1, 0),
            Point::new(960, 0),
            Point::new(0, 540),
            Point::new(i32::MAX, i32::MIN),
        ] {
            assert_eq!(rotation.to_portrait(point), None);
            assert_eq!(
                rotation.touch_to_portrait(TouchEvent::release(0, point)),
                TouchEvent::Cancel
            );
        }
        for point in [
            Point::new(-1, 0),
            Point::new(540, 0),
            Point::new(0, 960),
            Point::new(i32::MAX, i32::MIN),
        ] {
            assert_eq!(rotation.to_native(point), None);
        }
        assert_eq!(
            rotation.touch_to_portrait(TouchEvent::Cancel),
            TouchEvent::Cancel
        );
    }

    #[test]
    fn clear_busy_and_refresh_errors_are_forwarded() {
        let mut native = Panel::default();
        let mut portrait = PortraitDisplay::new(&mut native);
        portrait.clear(Gray4::WHITE).unwrap();
        assert!(!portrait.is_busy());
        portrait.refresh().unwrap();
        assert!(portrait.is_busy());
        assert_eq!(portrait.refresh(), Err("busy"));
        assert_eq!(portrait.clear(Gray4::BLACK), Err("busy"));
        assert_eq!(
            portrait.draw_iter([Pixel(Point::zero(), Gray4::BLACK)]),
            Err("busy")
        );
        assert_eq!(native.pixels, [15; 12]);
    }
}
