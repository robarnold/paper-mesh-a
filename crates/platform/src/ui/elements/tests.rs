use super::*;
use crate::ui::{
    Navigator, PageInfo, PageRuntime, Presentation, Shell, ShellLayout, TextFocus, TextInput,
};
const VIEW: Rectangle = Rectangle::new(Point::zero(), Size::new(540, 960));
const SHELL: ShellLayout = ShellLayout {
    viewport: VIEW,
    content: VIEW,
    context: Rectangle::zero(),
    title: Rectangle::zero(),
    divider: Rectangle::zero(),
    back: Rectangle::zero(),
};
struct Example {
    elements: Elements<Self, u16, 8, 32>,
    value: u16,
    focus: u16,
    fields: [u16; 2],
    label: Option<ElementId>,
}
impl Example {
    fn new() -> Self {
        Self {
            elements: Elements::new(VIEW),
            value: 0,
            focus: 0,
            fields: [0; 2],
            label: None,
        }
    }
}
impl Page for Example {
    type Id = u8;
    type Elements = Elements<Self, u16, 8, 32>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: "Test",
            context: "",
            presentation: Presentation::FullScreen,
            navigation_enabled: true,
        }
    }
    fn did_enter(&mut self) {
        if self.label.is_some() {
            return;
        }
        let root = self
            .elements
            .add(
                None,
                Layout {
                    flow: Flow::Column,
                    ..Layout::default()
                },
                Graphic::None,
                None,
            )
            .unwrap();
        let button = Button::create(
            &mut self.elements,
            root,
            "Apply",
            17,
            false,
            &Style::GRAYSCALE,
        )
        .unwrap();
        self.label = Some(button.label);
        button
            .did_activate(&mut self.elements, |app, ui, data| {
                app.value = data;
                ui.set_text(app.label.unwrap(), "Done").unwrap();
            })
            .unwrap();
    }
    fn text_focus(&self) -> Option<TextFocus> {
        Some(TextFocus {
            field: self.focus,
            keyboard: Rectangle::new(Point::new(28, 600), Size::new(484, 320)),
        })
    }
    fn text_input(&mut self, field: u16, _: TextInput) -> bool {
        self.fields[usize::from(field)] += 1;
        true
    }
}
// Non-capturing closures receive application state, typed control data, and a UI mutation context.
#[test]
fn callbacks_mutate_elements_after_input_traversal() {
    let mut ui = Navigator::<_, 0>::launch(0, Shell::new(SHELL), Example::new());
    ui.touch(crate::TouchEvent::touch(0, Point::new(40, 40)))
        .unwrap();
    ui.touch(crate::TouchEvent::release(0, Point::new(40, 40)))
        .unwrap();
    assert_eq!(ui.pages().value, 17);
    let id = ui.pages().label.unwrap();
    assert!(
        matches!(&ui.pages().elements.nodes[id.0].graphic,Graphic::Text{value,..} if value=="Done")
    );
}
// Focus transfer expires earlier keyboard sessions before either frame is presented.
#[test]
fn system_keyboard_focus_rejects_old_sessions() {
    let mut ui = Navigator::<_, 0>::launch(0, Shell::new(SHELL), Example::new());
    let old = ui.text_session().unwrap();
    assert!(ui.deliver_text(old, TextInput::Character('a')));
    ui.pages_mut().focus = 1;
    ui.prepare();
    assert!(!ui.deliver_text(old, TextInput::Character('b')));
    assert!(ui.deliver_text(ui.text_session().unwrap(), TextInput::Character('c')));
    assert_eq!(ui.pages().fields, [1, 1]);
}
// Layout failure preserves the last calculated bounds and does not publish partial geometry.
#[test]
fn layout_failure_preserves_bounds() {
    let mut page = Example::new();
    page.enter();
    let root = page.elements.root_element().unwrap();
    let label = page.label.unwrap();
    let before = page.elements.bounds(label).unwrap();
    page.elements
        .set_layout(
            root,
            Layout {
                flow: Flow::Column,
                padding: 1000,
                ..Layout::default()
            },
        )
        .unwrap();
    assert_eq!(page.prepare(), Err(ElementError::InsufficientSpace));
    assert_eq!(page.elements.bounds(label).unwrap(), before);
}
// Unchanged element batches preserve layer revision and keep display scheduling idle.
#[test]
fn unchanged_element_publication_is_a_noop() {
    let mut ui = Navigator::<_, 0>::launch(0, Shell::new(SHELL), Example::new());
    ui.submitted();
    ui.completed();
    let r = ui.layers().revision();
    ui.prepare();
    assert_eq!(ui.layers().revision(), r);
    assert!(!ui.layers().needs_display());
}

// A canceled ancestor pan reaches its callback and never activates the child button.
#[test]
fn pan_cancellation_reaches_the_page() {
    let mut ui = Navigator::<_, 0>::launch(0, Shell::new(SHELL), Example::new());
    let root = ui.pages().elements.root_element().unwrap();
    ui.pages_mut()
        .elements
        .did_pan(root, 0, |page, context, _, gesture| {
            page.value = match gesture {
                composition::Gesture::Pan {
                    phase: composition::PanPhase::Cancel,
                    ..
                } => 2,
                _ => 1,
            };
            context.changed();
        })
        .unwrap();
    ui.prepare();
    ui.touch(crate::TouchEvent::touch(0, Point::new(40, 40)))
        .unwrap();
    ui.touch(crate::TouchEvent::touch(0, Point::new(60, 40)))
        .unwrap();
    assert_eq!(ui.pages().value, 1);
    ui.touch(crate::TouchEvent::Cancel).unwrap();
    assert_eq!(ui.pages().value, 2);
    ui.touch(crate::TouchEvent::release(0, Point::new(40, 40)))
        .unwrap();
    assert_eq!(ui.pages().value, 2);
}

// Enabling a control during a held contact must not activate it on that contact's release.
#[test]
fn enabled_state_changes_require_a_new_tap() {
    let mut ui = Navigator::<_, 0>::launch(0, Shell::new(SHELL), Example::new());
    let label = ui.pages().label.unwrap();
    let element = ui.pages().elements.nodes[label.0].parent.unwrap();
    let button = Button { element, label };
    button
        .set_enabled(&mut ui.pages_mut().elements, EnabledState::from(false))
        .unwrap();
    ui.prepare();
    ui.touch(crate::TouchEvent::touch(0, Point::new(40, 40)))
        .unwrap();
    button
        .set_enabled(&mut ui.pages_mut().elements, EnabledState::from(true))
        .unwrap();
    ui.prepare();
    ui.touch(crate::TouchEvent::release(0, Point::new(40, 40)))
        .unwrap();
    assert_eq!(ui.pages().value, 0);
    ui.touch(crate::TouchEvent::touch(0, Point::new(40, 40)))
        .unwrap();
    ui.touch(crate::TouchEvent::release(0, Point::new(40, 40)))
        .unwrap();
    assert_eq!(ui.pages().value, 17);
}
