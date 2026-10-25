use super::*;
use crate::{TouchEvent, input::TapGesture};
use embedded_graphics::mock_display::MockDisplay;

#[test]
fn badges_grow_with_counts_and_keep_rounded_corners_and_contrasting_text() {
    let style = Style {
        foreground: Gray4::new(2),
        background: Gray4::new(14),
        ..Style::GRAYSCALE
    };
    for role in [TextRole::Body, TextRole::Caption, TextRole::Secondary] {
        let mut badge = TextBadge {
            position: Point::new(4, 4),
            label: "3",
            role,
            underlined: false,
        };
        let single = badge.bounds(&style);
        badge.label = "99+";
        let multiple = badge.bounds(&style);
        assert!(multiple.size.width > single.size.width);
        assert_eq!(multiple.size.height, single.size.height);

        let mut display = MockDisplay::new();
        display.set_allow_overdraw(true);
        badge.render(&mut display, &style).unwrap();
        assert_eq!(display.get_pixel(multiple.top_left), None);
        assert_eq!(
            display.get_pixel(Point::new(multiple.center().x, multiple.top_left.y)),
            Some(style.foreground)
        );
        assert!(
            multiple
                .points()
                .any(|p| display.get_pixel(p) == Some(style.background))
        );
        assert!(
            !multiple
                .points()
                .any(|p| display.get_pixel(p) == Some(style.secondary_text))
        );
        assert!(
            display
                .affected_area()
                .points()
                .all(|p| multiple.contains(p))
        );

        badge.label = "";
        assert_eq!(badge.bounds(&style).size, Size::zero());
        let mut empty = MockDisplay::new();
        badge.render(&mut empty, &style).unwrap();
        assert_eq!(empty.affected_area().size, Size::zero());
    }
}

struct BoundedTarget {
    allowed: [Rectangle; 4],
    pixels: usize,
}

impl OriginDimensions for BoundedTarget {
    fn size(&self) -> Size {
        Size::new(320, 240)
    }
}

impl DrawTarget for BoundedTarget {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(point, _) in pixels {
            assert!(
                self.allowed.iter().any(|bounds| bounds.contains(point)),
                "pixel outside component: {point:?}"
            );
            self.pixels += 1;
        }
        Ok(())
    }
}

#[test]
fn disabled_controls_cannot_arm_or_complete_a_tap() {
    let mut button = Button {
        bounds: Rectangle::new(Point::new(10, 20), Size::new(100, 50)),
        label: "Run",
        selected: false,
        enabled: false,
    };
    let mut gesture = TapGesture::new();
    let point = button.bounds.center();
    gesture.update(TouchEvent::touch(0, point), |p| button.hit(p).then_some(()));
    button.enabled = true;
    assert_eq!(
        gesture.update(TouchEvent::release(0, point), |p| button
            .hit(p)
            .then_some(())),
        None
    );
    gesture.update(TouchEvent::touch(0, point), |p| button.hit(p).then_some(()));
    button.enabled = false;
    assert_eq!(
        gesture.update(TouchEvent::release(0, point), |p| button
            .hit(p)
            .then_some(())),
        None
    );
}

#[test]
fn long_labels_and_small_controls_stay_inside_their_bounds() {
    for size in [Size::new(120, 80), Size::new(30, 12), Size::zero()] {
        let bounds = Rectangle::new(Point::new(17, 23), size);
        let mut target = BoundedTarget {
            allowed: [bounds; 4],
            pixels: 0,
        };
        let button = Button {
            bounds,
            label: "A very long control label",
            selected: true,
            enabled: true,
        };
        button.render(&mut target, &Style::GRAYSCALE).unwrap();
        let card = NavigationCard {
            bounds,
            title: "A very long component name",
            description: "A long description that must stay inside the card",
            enabled: false,
        };
        card.render(&mut target, &Style::GRAYSCALE).unwrap();
        assert!(!card.hit(bounds.top_left));
        if size == Size::zero() {
            assert_eq!(target.pixels, 0);
        }
    }
}

#[test]
fn header_text_cannot_draw_into_adjacent_navigation() {
    let context = Rectangle::new(Point::new(8, 8), Size::new(60, 12));
    let title = Rectangle::new(Point::new(8, 24), Size::new(60, 24));
    let divider = Rectangle::new(Point::new(8, 56), Size::new(180, 1));
    let control = Button {
        bounds: Rectangle::new(Point::new(104, 8), Size::new(84, 40)),
        label: "Exit",
        selected: false,
        enabled: true,
    };
    let mut target = BoundedTarget {
        allowed: [context, title, divider, control.bounds],
        pixels: 0,
    };
    Header {
        context: "A long context",
        title: "A very long page title",
        context_bounds: context,
        title_bounds: title,
        divider,
        controls: &[control],
    }
    .render(&mut target, &Style::GRAYSCALE)
    .unwrap();
    assert!(target.pixels > 0);
}
