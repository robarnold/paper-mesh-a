use super::*;
use crate::ui::{PageInfo, Presentation};

struct Example {
    elements: Elements<Self, (), 8, 0>,
}
impl Page for Example {
    type Id = u8;
    type Elements = Elements<Self, (), 8, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: "Layout",
            context: "",
            presentation: Presentation::FullScreen,
            navigation_enabled: true,
        }
    }
}
fn elements() -> Elements<Example, (), 8, 0> {
    Elements::new(Rectangle::new(Point::new(10, 20), Size::new(105, 55)))
}

// Rows and columns reserve fixed space and give remaining pixels to earlier fill children.
#[test]
fn linear_flows_distribute_remaining_pixels() {
    for (flow, expected) in [
        (
            Flow::Row,
            [(12, 22, 20, 51), (33, 22, 40, 51), (74, 22, 39, 51)],
        ),
        (
            Flow::Column,
            [(12, 22, 101, 20), (12, 43, 101, 15), (12, 59, 101, 14)],
        ),
    ] {
        let mut elements = elements();
        let root = elements
            .add(
                None,
                Layout {
                    flow,
                    padding: 2,
                    gap: 1,
                    ..Layout::default()
                },
                Graphic::None,
                None,
            )
            .unwrap();
        for i in 0..3 {
            let mut layout = Layout::default();
            if i == 0 {
                if flow == Flow::Row {
                    layout.width = Length::Fixed(20);
                } else {
                    layout.height = Length::Fixed(20);
                }
            }
            elements
                .add(Some(root), layout, Graphic::None, None)
                .unwrap();
        }
        elements.resolve(&Style::GRAYSCALE).unwrap();
        for (i, (x, y, w, h)) in expected.into_iter().enumerate() {
            assert_eq!(
                elements.bounds(ElementId(i + 1)).unwrap(),
                Rectangle::new(Point::new(x, y), Size::new(w, h))
            );
        }
    }
}

// Grid cells retain equal sizes; overlays apply offsets from the padded content origin.
#[test]
fn grid_and_overlay_have_distinct_placement() {
    let mut grid = elements();
    let root = grid
        .add(
            None,
            Layout {
                flow: Flow::Grid { columns: 2 },
                padding: 2,
                gap: 1,
                ..Layout::default()
            },
            Graphic::None,
            None,
        )
        .unwrap();
    for _ in 0..3 {
        grid.add(Some(root), Layout::default(), Graphic::None, None)
            .unwrap();
    }
    grid.resolve(&Style::GRAYSCALE).unwrap();
    for (i, point) in [Point::new(12, 22), Point::new(63, 22), Point::new(12, 48)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            grid.bounds(ElementId(i + 1)).unwrap(),
            Rectangle::new(point, Size::new(50, 25))
        );
    }
    let mut overlay = elements();
    let root = overlay
        .add(
            None,
            Layout {
                flow: Flow::Overlay,
                padding: 2,
                ..Layout::default()
            },
            Graphic::None,
            None,
        )
        .unwrap();
    let child = overlay
        .add(
            Some(root),
            Layout {
                offset: Point::new(7, 9),
                width: Length::Fixed(20),
                height: Length::Fixed(10),
                ..Layout::default()
            },
            Graphic::None,
            None,
        )
        .unwrap();
    overlay.resolve(&Style::GRAYSCALE).unwrap();
    assert_eq!(
        overlay.bounds(child).unwrap(),
        Rectangle::new(Point::new(19, 31), Size::new(20, 10))
    );
}
