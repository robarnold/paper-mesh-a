//! Native host for the guide's independent examples and catalog.
//!
//! # Design choices
//!
//! - Keep each guide example in a separate session so exploring a control cannot
//!   mutate the device application.
//!
//! See [UI guide](../../../docs/simulator/execution.md#ui-guide).

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use paper_mesh_platform::{
    TouchEvent,
    input::{
        buttons::{ButtonEvent, ButtonId, ButtonState},
        navigation::{ButtonRoute, NavigationBindings},
    },
    ui::{Navigation, Navigator, Page, Presentation, Shell},
};
use serde::{Deserialize, Serialize};

#[path = "../../../crates/app/examples/ui_guide/mod.rs"]
mod example;
pub use example::catalog::Sheet;
use example::{
    Example, LAYOUT, SIZE, STYLE,
    controls::{ControlsExample, Route as ControlsRoute},
    text::{COLORS, SPACING, TEXT_ROLES, TextExample},
};

pub const SOURCE: &str = include_str!("../../../crates/app/examples/ui_guide/mod.rs");
pub const CONTROLS_SOURCE: &str = include_str!("../../../crates/app/examples/ui_guide/controls.rs");
pub const TEXT_SOURCE: &str = include_str!("../../../crates/app/examples/ui_guide/text.rs");
pub const CATALOG_SOURCE: &str = include_str!("../../../crates/app/examples/ui_guide/catalog.rs");
pub const CALCULATOR_SOURCE: &str = include_str!("../../../crates/app/src/calculator.rs");

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    TouchDown { id: u8, x: i32, y: i32 },
    TouchMove { id: u8, x: i32, y: i32 },
    TouchUp { id: u8, x: i32, y: i32 },
    Cancel,
    Home,
    Finish,
    Reset,
}

#[derive(Serialize)]
pub struct Control {
    label: &'static str,
    x: i32,
    y: i32,
    enabled: bool,
}

#[derive(Serialize)]
pub struct State {
    page: &'static str,
    return_to: Option<&'static str>,
    back_stack: Vec<&'static str>,
    full_screen: bool,
    pending: bool,
    count: u32,
    selected: bool,
    home_entries: u32,
    detail_entries: u32,
    home_exits: u32,
    detail_exits: u32,
    transition: String,
    controls: Vec<Control>,
}

struct Session<E> {
    example: E,
    contacts: u16,
    suppressed: bool,
}

impl<E: Demo> Default for Session<E> {
    fn default() -> Self {
        let mut example = E::default();
        example.present();
        Self {
            example,
            contacts: 0,
            suppressed: false,
        }
    }
}

fn present<P: paper_mesh_platform::ui::PageSet, const N: usize>(ui: &mut Navigator<P, N>) {
    ui.prepare();
    ui.render(&mut Canvas::new(), &STYLE).unwrap();
    ui.submitted();
    ui.completed();
}

impl<E: Demo> Session<E> {
    fn command(&mut self, command: Command) -> Result<(), &'static str> {
        match command {
            Command::TouchDown { id, x, y }
            | Command::TouchMove { id, x, y }
            | Command::TouchUp { id, x, y } => {
                if id > 15
                    || !(0..SIZE.width as i32).contains(&x)
                    || !(0..SIZE.height as i32).contains(&y)
                {
                    return Err("Touch must have a valid contact ID and display position.");
                }
                let up = matches!(command, Command::TouchUp { .. });
                if up {
                    self.contacts &= !(1 << id);
                } else {
                    self.contacts |= 1 << id;
                }
                if !self.suppressed {
                    self.example.touch(if up {
                        TouchEvent::release(id, Point::new(x, y))
                    } else {
                        TouchEvent::touch(id, Point::new(x, y))
                    });
                }
                if self.contacts == 0 {
                    self.suppressed = false;
                }
            }
            Command::Cancel => {
                self.example.touch(TouchEvent::Cancel);
                self.contacts = 0;
                self.suppressed = false;
            }
            Command::Home => {
                self.example.touch(TouchEvent::Cancel);
                self.suppressed = self.contacts != 0;
                let bindings = NavigationBindings {
                    home: Some(ButtonId(0)),
                };
                if bindings.route(ButtonEvent {
                    id: ButtonId(0),
                    state: ButtonState::Pressed,
                    at_ms: 0,
                }) == ButtonRoute::Home
                {
                    self.example.home();
                }
            }
            Command::Finish => self.example.finish(),
            Command::Reset => *self = Self::default(),
        }
        self.example.present();
        Ok(())
    }
}

struct KeyboardExample {
    ui: Navigator<example::focus::FocusPage, 0>,
    revision: u64,
}
impl Default for KeyboardExample {
    fn default() -> Self {
        Self {
            ui: Navigator::launch(0, Shell::new(LAYOUT), example::focus::FocusPage::new()),
            revision: 0,
        }
    }
}
impl Demo for KeyboardExample {
    fn present(&mut self) {
        present(&mut self.ui);
    }
    fn touch(&mut self, event: TouchEvent) {
        if self.ui.touch(event).unwrap().changed {
            self.revision += 1;
        }
    }
    fn home(&mut self) {
        *self = Self::default();
    }
}

trait Demo: Default {
    fn present(&mut self) {}
    fn touch(&mut self, event: TouchEvent);
    fn home(&mut self);
    fn finish(&mut self) {}
}

impl Demo for Example {
    fn present(&mut self) {
        present(&mut self.ui);
    }
    fn touch(&mut self, event: TouchEvent) {
        Example::touch(self, event);
    }
    fn home(&mut self) {
        self.navigate(Navigation::Home);
    }
    fn finish(&mut self) {
        self.ui.pages_mut().detail.pending = false;
    }
}

impl Demo for ControlsExample {
    fn present(&mut self) {
        present(&mut self.ui);
    }
    fn touch(&mut self, event: TouchEvent) {
        let _ = self.ui.touch(event);
    }
    fn home(&mut self) {
        let _ = self.ui.navigate(Navigation::Home);
    }
    fn finish(&mut self) {
        self.ui.pages_mut().controls.pending = false;
    }
}

impl Demo for TextExample {
    fn present(&mut self) {
        present(&mut self.ui);
    }
    fn touch(&mut self, event: TouchEvent) {
        let _ = self.ui.touch(event);
    }
    fn home(&mut self) {
        let _ = self.ui.navigate(Navigation::Home);
    }
}

#[derive(Default)]
pub struct Guide {
    keyboard: Box<Session<KeyboardExample>>,
    composition: Box<Session<Example>>,
    controls: Box<Session<ControlsExample>>,
    text: Box<Session<TextExample>>,
    calculator: Box<Session<CalculatorExample>>,
}

struct CalculatorExample {
    ui: Navigator<paper_mesh_app::calculator::CalculatorPage, 0>,
}

impl Default for CalculatorExample {
    fn default() -> Self {
        let mut example = Self {
            ui: Navigator::launch(
                paper_mesh_app::pages::Route::Calculator,
                Shell::new(LAYOUT),
                paper_mesh_app::calculator::CalculatorPage::default(),
            ),
        };
        example.present();
        example
    }
}

impl CalculatorExample {
    fn present(&mut self) {
        // The guide has synchronous presentation. The device simulator tests delayed refreshes.
        present(&mut self.ui);
    }
}

impl Demo for CalculatorExample {
    fn present(&mut self) {
        present(&mut self.ui);
    }
    fn touch(&mut self, event: TouchEvent) {
        let update = self.ui.touch(event).unwrap();
        if update.changed {
            self.present();
        }
    }
    fn home(&mut self) {
        let _ = self.ui.navigate(Navigation::Home);
    }
}

#[derive(Serialize)]
pub struct CalculatorState {
    value: String,
    notice: String,
    controls: Vec<Control>,
}

#[derive(Serialize)]
pub struct TextState {
    role: &'static str,
    underlined: bool,
    bubbled: bool,
    spacing: &'static str,
    gap: u32,
    color: &'static str,
    shade: u8,
    controls: Vec<Control>,
}

#[derive(Serialize)]
pub struct ControlsState {
    page: &'static str,
    count: u32,
    selected: bool,
    pending: bool,
    controls: Vec<Control>,
}

impl Guide {
    pub fn calculator_command(&mut self, command: Command) -> Result<(), &'static str> {
        self.calculator.command(command)
    }

    pub fn calculator_state(&self) -> CalculatorState {
        let page = self.calculator.example.ui.pages();
        CalculatorState {
            value: page.value().into(),
            notice: page.notice().into(),
            controls: paper_mesh_app::calculator::KEYS
                .iter()
                .enumerate()
                .map(|(i, (_, label))| {
                    let center = page.key_bounds(i).center();
                    Control {
                        label,
                        x: center.x,
                        y: center.y,
                        enabled: page.input_ready(),
                    }
                })
                .collect(),
        }
    }

    pub fn calculator_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut canvas = Canvas::new();
        self.calculator
            .example
            .ui
            .render(&mut canvas, &STYLE)
            .unwrap();
        canvas.png()
    }

    pub fn keyboard_command(&mut self, command: Command) -> Result<(), &'static str> {
        self.keyboard.command(command)
    }

    pub fn keyboard_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut canvas = Canvas::new();
        self.keyboard
            .example
            .ui
            .render(&mut canvas, &STYLE)
            .unwrap();
        canvas.png()
    }

    pub fn keyboard_state(&self) -> serde_json::Value {
        serde_json::json!({ "revision": self.keyboard.example.revision })
    }

    pub fn command(&mut self, command: Command) -> Result<(), &'static str> {
        self.composition.command(command)
    }

    pub fn controls_command(&mut self, command: Command) -> Result<(), &'static str> {
        self.controls.command(command)
    }

    pub fn text_command(&mut self, command: Command) -> Result<(), &'static str> {
        self.text.command(command)
    }

    pub fn text_state(&self) -> TextState {
        let page = self.text.example.ui.pages();
        TextState {
            role: TEXT_ROLES[page.role].1,
            underlined: page.underlined,
            bubbled: page.bubbled,
            spacing: SPACING[page.spacing].0,
            gap: SPACING[page.spacing].1,
            color: COLORS[page.color].0,
            shade: COLORS[page.color].1.luma(),
            controls: page
                .role_buttons()
                .into_iter()
                .chain([page.underline_button()])
                .chain([page.bubble_button()])
                .chain(page.spacing_buttons())
                .chain(page.swatches())
                .map(|button| Control {
                    label: button.label,
                    x: button.bounds.center().x,
                    y: button.bounds.center().y,
                    enabled: button.enabled,
                })
                .collect(),
        }
    }

    pub fn text_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut canvas = Canvas::new();
        self.text.example.ui.render(&mut canvas, &STYLE).unwrap();
        canvas.png()
    }

    pub fn controls_state(&self) -> ControlsState {
        let example = &self.controls.example;
        let page = &example.ui.pages().controls;
        let mut controls = Vec::new();
        let mut add = |label, bounds: embedded_graphics::primitives::Rectangle, enabled| {
            controls.push(Control {
                label,
                x: bounds.center().x,
                y: bounds.center().y,
                enabled,
            });
        };
        if let Some(back) = example.ui.back_control() {
            add(back.label, back.bounds, back.enabled);
        }
        if example.ui.current() == ControlsRoute::Controls {
            for button in page.buttons() {
                add(button.label, button.bounds, button.enabled);
            }
            let card = page.card();
            add(card.title, card.bounds, card.enabled);
            let finish = page.finish_button();
            add(finish.label, finish.bounds, finish.enabled);
        }
        ControlsState {
            page: example.ui.current().title(),
            count: page.count,
            selected: page.selected,
            pending: page.pending,
            controls,
        }
    }

    pub fn controls_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut canvas = Canvas::new();
        self.controls
            .example
            .ui
            .render(&mut canvas, &STYLE)
            .unwrap();
        canvas.png()
    }

    pub fn state(&self) -> State {
        let page = self.composition.example.page();
        let mut controls = Vec::new();
        let mut add = |label, bounds: embedded_graphics::primitives::Rectangle, enabled| {
            controls.push(Control {
                label,
                x: bounds.center().x,
                y: bounds.center().y,
                enabled,
            })
        };
        if let Some(back) = self.composition.example.ui.back_control() {
            add(back.label, back.bounds, back.enabled);
        }
        if page.route != example::Route::Detail {
            let card = page.card();
            add(card.title, card.bounds, card.enabled);
            if page.route == example::Route::Home {
                let card = page.tools_card();
                add(card.title, card.bounds, card.enabled);
            }
        } else {
            for button in page.buttons() {
                add(button.label, button.bounds, button.enabled);
            }
        }
        State {
            page: page.route.title(),
            return_to: self
                .composition
                .example
                .ui
                .return_to()
                .map(example::Route::title),
            back_stack: self
                .composition
                .example
                .ui
                .history()
                .iter()
                .map(|route| route.title())
                .collect(),
            full_screen: page.page_info().presentation == Presentation::FullScreen,
            pending: page.pending,
            count: self.composition.example.ui.pages().detail.count,
            selected: self.composition.example.ui.pages().detail.selected,
            home_entries: self.composition.example.ui.pages().home.entries,
            detail_entries: self.composition.example.ui.pages().detail.entries,
            home_exits: self.composition.example.ui.pages().home.exits,
            detail_exits: self.composition.example.ui.pages().detail.exits,
            transition: self.composition.example.transition.map_or_else(
                || "Home.enter".into(),
                |t| format!("{}.leave → {}.enter", t.from.title(), t.to.title()),
            ),
            controls,
        }
    }

    pub fn png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut canvas = Canvas::new();
        self.composition.example.render(&mut canvas).unwrap();
        canvas.png()
    }
}

pub fn catalog_png(sheet: Sheet<'_>) -> Result<Vec<u8>, png::EncodingError> {
    let mut canvas = Canvas::new();
    Navigator::<_, 0>::launch(0, Shell::new(LAYOUT), example::catalog::Catalog::new(sheet))
        .render(&mut canvas, &STYLE)
        .unwrap();
    canvas.png()
}

/// Extract the compiled example's marked methods for the browser guide.
pub fn snippets() -> serde_json::Value {
    let mut snippets = serde_json::Map::new();
    for (source, names) in [
        (
            SOURCE,
            &[
                "page",
                "lifecycle",
                "composition",
                "navigation",
                "input",
                "render",
            ][..],
        ),
        (CONTROLS_SOURCE, &["button", "card"][..]),
        (TEXT_SOURCE, &["text"][..]),
    ] {
        for name in names {
            let start = format!("// guide:{name}:start");
            let end = format!("// guide:{name}:end");
            let text = source
                .split_once(&start)
                .unwrap()
                .1
                .split_once(&end)
                .unwrap()
                .0
                .trim_matches('\n')
                .trim_end();
            let indent = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.len() - line.trim_start().len())
                .min()
                .unwrap_or(0);
            let text = text
                .lines()
                .map(|line| line.get(indent..).unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            snippets.insert((*name).into(), text.into());
        }
    }
    snippets.into()
}

struct Canvas(Vec<u8>);
impl Canvas {
    fn new() -> Self {
        Self(vec![255; (SIZE.width * SIZE.height) as usize])
    }
    fn png(self) -> Result<Vec<u8>, png::EncodingError> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, SIZE.width, SIZE.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&self.0)?;
        Ok(bytes)
    }
}
impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        SIZE
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
            if self.bounding_box().contains(point) {
                self.0[point.y as usize * SIZE.width as usize + point.x as usize] =
                    color.luma() * 17;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
