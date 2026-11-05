//! Decimal calculator and its retained UI composition.
//!
//! # Design choices
//!
//! - Use bounded decimal arithmetic so results do not depend on binary float formatting.
//! - Keep arithmetic outside UI elements so graphics and input remain reusable.
//!
//! See [calculator behavior](../../../docs/application.md#calculator).

use crate::{
    pages::Route,
    screen::{self, STYLE},
};
use core::fmt::Write;
use embedded_graphics::{prelude::*, primitives::Rectangle};
use heapless::String;
use paper_mesh_platform::ui::{
    Page, PageEvent, PageInfo, TextRole,
    elements::{ElementId, Elements, Graphic},
    layout::{Flow, Layout, Length},
};

pub const HOME_TARGET: Rectangle = Rectangle::new(Point::new(28, 502), Size::new(484, 112));
const SCALE: i64 = 1_000_000;
const LIMIT: i64 = 999_999_999_999_999;
const INPUT_HINT: &str = "Up to 9 whole and 6 decimal digits";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Decimal,
    Sign,
    Delete,
    Clear,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equals,
}

pub const KEYS: [(Key, &str); 19] = [
    (Key::Clear, "C"),
    (Key::Delete, "Del"),
    (Key::Sign, "+/-"),
    (Key::Divide, "/"),
    (Key::Digit(7), "7"),
    (Key::Digit(8), "8"),
    (Key::Digit(9), "9"),
    (Key::Multiply, "x"),
    (Key::Digit(4), "4"),
    (Key::Digit(5), "5"),
    (Key::Digit(6), "6"),
    (Key::Subtract, "-"),
    (Key::Digit(1), "1"),
    (Key::Digit(2), "2"),
    (Key::Digit(3), "3"),
    (Key::Add, "+"),
    (Key::Digit(0), "0"),
    (Key::Decimal, "."),
    (Key::Equals, "="),
];

struct Calculator {
    entry: String<24>,
    accumulator: Option<i64>,
    operator: Option<Key>,
    entering: bool,
    failed: bool,
    notice: &'static str,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    const fn new() -> Self {
        Self {
            entry: String::new(),
            accumulator: None,
            operator: None,
            entering: true,
            failed: false,
            notice: INPUT_HINT,
        }
    }

    fn value(&self) -> &str {
        if self.entry.is_empty() {
            "0"
        } else {
            &self.entry
        }
    }
    fn notice(&self) -> &str {
        self.notice
    }

    fn press(&mut self, key: Key) {
        if key == Key::Clear {
            *self = Self::default();
            return;
        }
        if self.failed {
            if matches!(key, Key::Digit(_) | Key::Decimal) {
                *self = Self::default();
            } else {
                return;
            }
        }
        match key {
            Key::Digit(digit) if digit < 10 => {
                self.start_entry();
                let unsigned = self.entry.trim_start_matches('-');
                let full = match unsigned.split_once('.') {
                    Some((_, fraction)) => fraction.len() >= 6,
                    None => unsigned.len() >= 9,
                };
                if full {
                    self.notice = "Input limit: 9 whole, 6 decimal digits";
                    return;
                }
                if unsigned == "0" {
                    self.entry.pop();
                }
                self.entry.push(char::from(b'0' + digit)).unwrap();
                self.notice = INPUT_HINT;
            }
            Key::Decimal => {
                self.start_entry();
                if !self.entry.contains('.') {
                    self.entry.push('.').unwrap();
                }
                self.notice = INPUT_HINT;
            }
            Key::Sign => {
                if self.entry.is_empty() {
                    self.entry.push('0').unwrap();
                }
                if !self.entering && self.operator.is_some() {
                    self.start_entry();
                }
                let mut next = String::new();
                if let Some(unsigned) = self.entry.strip_prefix('-') {
                    next.push_str(unsigned).unwrap();
                } else {
                    write!(next, "-{}", self.entry).unwrap();
                }
                self.entry = next;
            }
            Key::Delete => {
                self.start_entry();
                self.entry.pop();
                if self.entry.is_empty() || self.entry == "-" {
                    self.entry = String::try_from("0").unwrap();
                }
                self.notice = INPUT_HINT;
            }
            Key::Add | Key::Subtract | Key::Multiply | Key::Divide => {
                if self.operator.is_some() && self.entering && !self.calculate() {
                    return;
                }
                if self.operator.is_none() {
                    self.accumulator = Some(self.number());
                }
                self.operator = Some(key);
                self.entering = false;
                self.notice = match key {
                    Key::Add => "Add: enter the next number",
                    Key::Subtract => "Subtract: enter the next number",
                    Key::Multiply => "Multiply: enter the next number",
                    _ => "Divide: enter the next number",
                };
            }
            Key::Equals => {
                if self.operator.is_none() {
                    return;
                }
                if !self.entering {
                    self.notice = "Enter the next number before equals";
                    return;
                }
                if self.calculate() {
                    self.operator = None;
                    self.accumulator = None;
                    self.notice = "Result (rounded to 6 decimal places)";
                }
            }
            _ => {}
        }
    }

    fn start_entry(&mut self) {
        if !self.entering || self.entry.is_empty() {
            self.entry = String::try_from("0").unwrap();
            self.entering = true;
        }
    }

    fn number(&self) -> i64 {
        let mut value = 0;
        let mut fraction = None;
        for byte in self.entry.bytes() {
            match byte {
                b'0'..=b'9' => {
                    value = value * 10 + i64::from(byte - b'0');
                    if let Some(n) = &mut fraction {
                        *n += 1;
                    }
                }
                b'.' => fraction = Some(0),
                _ => {}
            }
        }
        for _ in fraction.unwrap_or(0)..6 {
            value *= 10;
        }
        if self.entry.starts_with('-') {
            -value
        } else {
            value
        }
    }

    fn calculate(&mut self) -> bool {
        let a = i128::from(self.accumulator.unwrap_or(0));
        let b = i128::from(self.number());
        let value = match self.operator.unwrap() {
            Key::Add => a + b,
            Key::Subtract => a - b,
            Key::Multiply => rounded_divide(a * b, i128::from(SCALE)),
            Key::Divide if b != 0 => rounded_divide(a * i128::from(SCALE), b),
            _ => {
                self.failed = true;
                self.notice = "Cannot divide by zero. Clear or type a digit.";
                return false;
            }
        };
        if value.abs() > i128::from(LIMIT) {
            self.failed = true;
            self.notice = "Numeric limit. Clear or type a digit.";
            return false;
        }
        let value = value as i64;
        self.accumulator = Some(value);
        self.entry.clear();
        if value < 0 {
            self.entry.push('-').unwrap();
        }
        write!(self.entry, "{}", value.abs() / SCALE).unwrap();
        if value % SCALE != 0 {
            write!(self.entry, ".{:06}", value.abs() % SCALE).unwrap();
            while self.entry.ends_with('0') {
                self.entry.pop();
            }
        }
        self.entering = false;
        true
    }
}

fn rounded_divide(a: i128, b: i128) -> i128 {
    let quotient = a / b;
    if (a % b).abs() * 2 >= b.abs() {
        quotient + if (a < 0) == (b < 0) { 1 } else { -1 }
    } else {
        quotient
    }
}

pub struct CalculatorPage {
    model: Calculator,
    elements: Elements<CalculatorPage, Key, 48, 64>,
    value: Option<ElementId>,
    status: Option<ElementId>,
    keys: [Option<ElementId>; 19],
}

impl Default for CalculatorPage {
    fn default() -> Self {
        let mut page = Self::empty();
        page.initialize();
        page
    }
}

impl CalculatorPage {
    pub(crate) const fn empty() -> Self {
        Self {
            model: Calculator::new(),
            elements: Elements::new(screen::CONTENT_BOUNDS),
            value: None,
            status: None,
            keys: [None; 19],
        }
    }

    pub(crate) fn initialize(&mut self) {
        // Startup owns this one-time call. Repeating it would append duplicate elements and lose retained element identities.
        assert!(self.value.is_none(), "calculator is already initialized");
        let elements = &mut self.elements;
        let root = elements
            .add(
                None,
                Layout {
                    flow: Flow::Column,
                    gap: STYLE.spacing.control_gap,
                    ..Layout::default()
                },
                Graphic::None,
                None,
            )
            .unwrap();
        let readout = elements
            .add(
                Some(root),
                Layout {
                    flow: Flow::Column,
                    height: Length::Fixed(140),
                    padding: STYLE.spacing.control_inset,
                    gap: 8,
                    ..Layout::default()
                },
                Graphic::Solid(STYLE.control_fill),
                None,
            )
            .unwrap();
        let value = elements
            .add(
                Some(readout),
                Layout {
                    height: Length::Fill,
                    ..Layout::default()
                },
                Graphic::text(self.model.value(), TextRole::Title, STYLE.foreground, false)
                    .unwrap(),
                None,
            )
            .unwrap();
        let status = elements
            .add(
                Some(readout),
                Layout {
                    height: Length::Fixed(30),
                    ..Layout::default()
                },
                Graphic::text(
                    self.model.notice(),
                    TextRole::Caption,
                    STYLE.secondary_text,
                    false,
                )
                .unwrap(),
                None,
            )
            .unwrap();
        let grid = elements
            .add(
                Some(root),
                Layout {
                    flow: Flow::Grid { columns: 4 },
                    gap: STYLE.spacing.control_gap,
                    ..Layout::default()
                },
                Graphic::None,
                None,
            )
            .unwrap();
        let keys = core::array::from_fn(|i| {
            // Leave a neutral cell before Equals without adding a second Clear action.
            if i == 18 {
                elements
                    .add(Some(grid), Layout::default(), Graphic::None, None)
                    .unwrap();
            }
            let (action, label) = KEYS[i];
            let primary = action == Key::Equals;
            let button = paper_mesh_platform::ui::elements::Button::create(
                elements, grid, label, action, primary, &STYLE,
            )
            .unwrap();
            button
                .did_activate(elements, |page, context, key| {
                    context.respond(PageEvent::changed(page.press(key)));
                })
                .unwrap();
            Some(button.element)
        });
        elements.resolve(&STYLE).unwrap();
        self.value = Some(value);
        self.status = Some(status);
        self.keys = keys;
    }

    pub fn value(&self) -> &str {
        self.model.value()
    }
    pub fn notice(&self) -> &str {
        self.model.notice()
    }
    pub fn input_ready(&self) -> bool {
        self.elements.input_ready()
    }
    pub fn key_bounds(&self, index: usize) -> Rectangle {
        self.elements.bounds(self.keys[index].unwrap()).unwrap()
    }
    fn press(&mut self, key: Key) -> bool {
        self.model.press(key);
        self.elements
            .set_graphic(
                self.value.unwrap(),
                Graphic::text(self.model.value(), TextRole::Title, STYLE.foreground, false)
                    .unwrap(),
            )
            .unwrap();
        self.elements
            .set_graphic(
                self.status.unwrap(),
                Graphic::text(
                    self.model.notice(),
                    TextRole::Caption,
                    STYLE.secondary_text,
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        self.elements.damage().is_some()
    }
}

impl Page for CalculatorPage {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Calculator.info()
    }
    type Elements = Elements<Self, Key, 48, 64>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
}

#[cfg(test)]
mod tests;
