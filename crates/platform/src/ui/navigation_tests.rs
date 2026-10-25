use super::*;
use crate::ui::{Page, Presentation, ShellLayout};
use embedded_graphics::primitives::Rectangle;

const LAYOUT: ShellLayout = ShellLayout {
    viewport: Rectangle::new(Point::zero(), Size::new(64, 64)),
    content: Rectangle::new(Point::new(0, 20), Size::new(64, 44)),
    context: Rectangle::new(Point::zero(), Size::new(30, 10)),
    title: Rectangle::new(Point::new(0, 10), Size::new(30, 10)),
    divider: Rectangle::new(Point::new(0, 19), Size::new(64, 1)),
    back: Rectangle::new(Point::new(40, 0), Size::new(24, 18)),
};

struct Pages {
    lifecycle: Vec<(u8, bool), 64>,
    touches: Vec<(u8, TouchEvent), 64>,
    pending: bool,
    full_screen: bool,
    open_on_touch: Option<u8>,
}

impl PageSet for Pages {
    type Id = u8;

    fn info(&self, id: u8) -> PageInfo<u8> {
        PageInfo {
            id,
            title: "Title",
            context: "Context",
            presentation: if self.full_screen {
                Presentation::FullScreen
            } else {
                Presentation::Standard
            },
            navigation_enabled: !self.pending,
        }
    }
    fn enter_page(&mut self, id: u8) {
        self.lifecycle.push((id, true)).unwrap();
    }
    fn leave_page(&mut self, id: u8) {
        self.lifecycle.push((id, false)).unwrap();
    }
    fn touch_page(&mut self, id: u8, event: TouchEvent) -> PageEvent<u8> {
        self.touches.push((id, event)).unwrap();
        if matches!(event, TouchEvent::Touch(_)) {
            return self.open_on_touch.take().map_or(PageEvent::None, |id| {
                PageEvent::Navigate(Navigation::Open(id))
            });
        }
        PageEvent::None
    }
    fn draw_page<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u8,
        target: &mut D,
    ) -> Result<(), D::Error> {
        target.clear(Gray4::BLACK)
    }
}

fn launch<const N: usize>() -> Navigator<Pages, N> {
    Navigator::launch(
        0,
        Shell::new(LAYOUT),
        Pages {
            lifecycle: Vec::new(),
            touches: Vec::new(),
            pending: false,
            full_screen: false,
            open_on_touch: None,
        },
    )
}

#[test]
fn one_page_returns_to_each_caller_and_home_clears_history() {
    let mut ui = launch::<4>();
    assert_eq!(ui.pages().lifecycle.as_slice(), &[(0, true)]);
    assert!(ui.back_control().is_none());
    assert_eq!(ui.navigate(Navigation::Back), Ok(None));
    assert_eq!(ui.navigate(Navigation::Home), Ok(None));
    for caller in [1, 2] {
        ui.navigate(Navigation::Open(caller)).unwrap();
        ui.navigate(Navigation::Open(3)).unwrap();
        assert_eq!(ui.return_to(), Some(caller));
        assert_eq!(
            ui.navigate(Navigation::Back),
            Ok(Some(Transition {
                from: 3,
                to: caller
            }))
        );
        assert_eq!(
            ui.navigate(Navigation::Back),
            Ok(Some(Transition {
                from: caller,
                to: 0
            }))
        );
    }
    assert_eq!(
        &ui.pages().lifecycle[..9],
        &[
            (0, true),
            (0, false),
            (1, true),
            (1, false),
            (3, true),
            (3, false),
            (1, true),
            (1, false),
            (0, true),
        ]
    );
    ui.navigate(Navigation::Open(1)).unwrap();
    ui.navigate(Navigation::Open(3)).unwrap();
    ui.navigate(Navigation::Home).unwrap();
    assert_eq!(ui.return_to(), None);
    assert_eq!(ui.navigate(Navigation::Back), Ok(None));
    ui.navigate(Navigation::Open(3)).unwrap();
    assert_eq!(ui.return_to(), Some(0));
    ui.navigate(Navigation::Open(0)).unwrap();
    assert_eq!(ui.return_to(), None);
}

#[test]
fn repeated_destinations_use_the_invocation_stack() {
    let mut ui = launch::<4>();
    assert!(ui.history().is_empty());
    for id in [1, 2, 1] {
        ui.navigate(Navigation::Open(id)).unwrap();
    }
    let callbacks = ui.pages().lifecycle.len();
    assert_eq!(ui.history(), &[0, 1, 2]);
    assert_eq!(ui.navigate(Navigation::Open(1)), Ok(None));
    assert_eq!(ui.pages().lifecycle.len(), callbacks);
    assert_eq!(ui.history(), &[0, 1, 2]);
    for (id, remaining) in [(2, &[0, 1][..]), (1, &[0][..]), (0, &[][..])] {
        ui.navigate(Navigation::Back).unwrap();
        assert_eq!(ui.current(), id);
        assert_eq!(ui.history(), remaining);
    }
}

#[test]
fn full_history_rejects_open_without_lifecycle_or_history_changes() {
    let mut ui = launch::<1>();
    ui.navigate(Navigation::Open(1)).unwrap();
    let callbacks = ui.pages().lifecycle.len();
    assert_eq!(
        ui.navigate(Navigation::Open(2)),
        Err(NavigationError::HistoryFull)
    );
    assert_eq!((ui.current(), ui.return_to()), (1, Some(0)));
    assert_eq!(ui.pages().lifecycle.len(), callbacks);
    ui.navigate(Navigation::Home).unwrap();
    ui.navigate(Navigation::Open(2)).unwrap();
    assert_eq!(ui.current(), 2);
    let mut root_only = launch::<0>();
    assert_eq!(
        root_only.navigate(Navigation::Open(1)),
        Err(NavigationError::HistoryFull)
    );
    assert_eq!(root_only.current(), 0);
}

#[test]
fn pending_and_transitions_cannot_complete_an_old_back_tap() {
    let mut ui = launch::<4>();
    ui.navigate(Navigation::Open(1)).unwrap();
    ui.pages_mut().pending = true;
    assert!(!ui.back_control().unwrap().enabled);
    let callbacks = ui.pages().lifecycle.len();
    for request in [Navigation::Home, Navigation::Back, Navigation::Open(2)] {
        assert_eq!(ui.navigate(request), Ok(None));
    }
    assert_eq!(ui.pages().lifecycle.len(), callbacks);
    let point = LAYOUT.back.center();
    ui.touch(TouchEvent::touch(0, point)).unwrap();
    ui.pages_mut().pending = false;
    assert!(!ui.touch(TouchEvent::release(0, point)).unwrap().changed);
    ui.touch(TouchEvent::touch(0, point)).unwrap();
    ui.navigate(Navigation::Open(2)).unwrap();
    assert!(!ui.touch(TouchEvent::release(0, point)).unwrap().changed);
    ui.touch(TouchEvent::touch(0, point)).unwrap();
    assert_eq!(
        ui.touch(TouchEvent::release(0, point)).unwrap().transition,
        Some(Transition { from: 2, to: 1 })
    );
}

#[test]
fn page_requests_run_callbacks_and_suppress_contacts_until_release() {
    let mut ui = launch::<4>();
    let point = Point::new(10, 30);
    ui.pages_mut().open_on_touch = Some(1);
    assert_eq!(
        ui.touch(TouchEvent::touch(0, point)).unwrap().transition,
        Some(Transition { from: 0, to: 1 })
    );
    ui.touch(TouchEvent::touch(1, point)).unwrap();
    ui.touch(TouchEvent::release(0, point)).unwrap();
    ui.touch(TouchEvent::release(1, point)).unwrap();
    assert!(!ui.pages().touches.iter().any(|(id, _)| *id == 1));
    ui.touch(TouchEvent::touch(0, point)).unwrap();
    assert_eq!(
        ui.pages().touches.last(),
        Some(&(1, TouchEvent::touch(0, point)))
    );
    ui.navigate(Navigation::Home).unwrap();
    ui.touch(TouchEvent::Cancel).unwrap();
    ui.pages_mut().open_on_touch = Some(2);
    ui.touch(TouchEvent::touch(0, point)).unwrap();
    assert_eq!(ui.current(), 2);
}

struct Canvas([Gray4; 64 * 64]);
impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        Size::new(64, 64)
    }
}
impl DrawTarget for Canvas {
    type Color = Gray4;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(point, color) in pixels {
            assert!(self.bounding_box().contains(point));
            self.0[point.y as usize * 64 + point.x as usize] = color;
        }
        Ok(())
    }
}

#[test]
fn shell_clips_content_and_full_screen_keeps_home_navigation() {
    let mut ui = launch::<4>();
    let mut canvas = Canvas([Gray4::new(3); 64 * 64]);
    ui.render(&mut canvas, &Style::GRAYSCALE).unwrap();
    assert_eq!(canvas.0[5 * 64 + 50], Gray4::WHITE);
    assert!(
        canvas.0[20 * 64..]
            .iter()
            .all(|color| *color == Gray4::BLACK)
    );
    ui.navigate(Navigation::Open(1)).unwrap();
    ui.render(&mut canvas, &Style::GRAYSCALE).unwrap();
    assert_eq!(canvas.0[64 + 41], Style::GRAYSCALE.control_fill);
    ui.pages_mut().full_screen = true;
    assert!(ui.back_control().is_none());
    ui.render(&mut canvas, &Style::GRAYSCALE).unwrap();
    assert!(canvas.0.iter().all(|color| *color == Gray4::BLACK));
    assert!(ui.navigate(Navigation::Home).unwrap().is_some());
}

struct SinglePage(u8);
impl Page for SinglePage {
    type Id = u8;
    fn page_info(&self) -> PageInfo<u8> {
        launch::<0>().page_info()
    }
    fn enter(&mut self) {
        self.0 += 1;
    }
    fn leave(&mut self) {
        panic!("a root-only page cannot leave");
    }
    fn handle_touch(&mut self, _: TouchEvent) -> PageEvent<u8> {
        PageEvent::None
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, _: &mut D) -> Result<(), D::Error> {
        Ok(())
    }
}

#[test]
fn a_single_page_can_launch_without_a_collection_adapter() {
    let ui = Navigator::<_, 0>::launch(0, Shell::new(LAYOUT), SinglePage(0));
    assert_eq!(ui.pages().0, 1);
}
