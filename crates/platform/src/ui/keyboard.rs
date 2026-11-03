//! Standard keyboard pages and their visible key bounds.

use super::{Style, TextRole};
use crate::{TouchEvent, input::TapGesture};
use embedded_graphics::{
    image::Image,
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use embedded_iconoir::{icons::size24px, prelude::IconoirNewIcon};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeyboardPage {
    Letters,
    ShiftedLetters,
    CommonSymbols,
    AdditionalSymbols,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Key {
    Character(char),
    Backspace,
    Shift,
    ShowLetters,
    ShowCommonSymbols,
    ShowAdditionalSymbols,
}

pub struct SoftKeyboard {
    bounds: Rectangle,
    gesture: TapGesture<Key>,
    page: KeyboardPage,
}

impl SoftKeyboard {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            page: KeyboardPage::Letters,
            gesture: TapGesture::new(),
        }
    }

    /// Apply a page change on release of a layout key.
    pub fn handle_touch(&mut self, event: TouchEvent) -> bool {
        let hit = match event {
            TouchEvent::Touch(contact) | TouchEvent::Release(contact) => self
                .keys()
                .find(|(_, bounds)| bounds.contains(contact.position))
                .map(|(key, _)| key),
            TouchEvent::Cancel => None,
        };
        let Some(key) = self.gesture.update(event, |_| hit) else {
            return false;
        };

        use KeyboardPage::{AdditionalSymbols, CommonSymbols, Letters, ShiftedLetters};
        let previous = self.page;
        self.page = match (previous, key) {
            (Letters, Key::Shift) => ShiftedLetters,
            (ShiftedLetters, Key::Shift) | (_, Key::ShowLetters) => Letters,
            (_, Key::ShowCommonSymbols) => CommonSymbols,
            (CommonSymbols, Key::ShowAdditionalSymbols) => AdditionalSymbols,
            _ => self.page,
        };
        previous != self.page
    }

    /// Each item supplies the key identity and its visible touch target.
    fn keys(&self) -> impl Iterator<Item = (Key, Rectangle)> + '_ {
        use KeyboardPage::{AdditionalSymbols, CommonSymbols, Letters, ShiftedLetters};
        let rows = match self.page {
            Letters => ["qwertyuiop", "asdfghjkl", "zxcvbnm"],
            ShiftedLetters => ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"],
            CommonSymbols => ["1234567890", "@#$%&-+()", ".?!'\":/"],
            AdditionalSymbols => ["1234567890", "[]{}<>*=_", "\\|~^`;:"],
        };
        let third = match self.page {
            Letters | ShiftedLetters => Key::Shift,
            CommonSymbols => Key::ShowAdditionalSymbols,
            AdditionalSymbols => Key::ShowCommonSymbols,
        };
        let mode = if matches!(self.page, Letters | ShiftedLetters) {
            Key::ShowCommonSymbols
        } else {
            Key::ShowLetters
        };
        let letters = rows.into_iter().enumerate().flat_map(|(row, chars)| {
            chars.chars().enumerate().map(move |(column, c)| {
                let offset = [0, 1, 3][row];
                (Key::Character(c), row as u32, offset + 2 * column as u32, 2)
            })
        });
        letters
            .chain([
                (third, 2, 1, 2),
                (Key::Backspace, 2, 17, 2),
                (mode, 3, 0, 4),
                (Key::Character(','), 3, 4, 2),
                (Key::Character(' '), 3, 6, 14),
            ])
            .map(|(key, row, start, width)| {
                // Twenty horizontal units allow a half-key indent on the shorter rows.
                let x =
                    |unit: u32| (u64::from(self.bounds.size.width) * u64::from(unit) / 20) as u32;
                let y =
                    |unit: u32| (u64::from(self.bounds.size.height) * u64::from(unit) / 4) as u32;
                let cell = Rectangle::new(
                    self.bounds.top_left + Point::new(x(start) as i32, y(row) as i32),
                    Size::new(x(start + width) - x(start), y(row + 1) - y(row)),
                );
                (key, cell.offset(-2))
            })
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        for (key, bounds) in self.keys() {
            let mut target = display.clipped(&bounds);
            let selected = key == Key::Shift && self.page == KeyboardPage::ShiftedLetters;
            let (fill, ink) = if selected {
                (style.foreground, style.background)
            } else {
                (style.control_fill, style.foreground)
            };
            bounds
                .into_styled(PrimitiveStyle::with_fill(fill))
                .draw(&mut target)?;
            let origin = bounds.center() - Point::new(11, 11);
            let mut buffer = [0; 4];
            let label = match key {
                Key::Shift => {
                    Image::new(&size24px::navigation::ArrowUp::new(ink), origin)
                        .draw(&mut target)?;
                    continue;
                }
                Key::Backspace => {
                    Image::new(&size24px::actions::Erase::new(ink), origin).draw(&mut target)?;
                    continue;
                }
                Key::ShowCommonSymbols if self.page == KeyboardPage::AdditionalSymbols => "@#+",
                Key::ShowCommonSymbols => "123",
                Key::ShowAdditionalSymbols => "[]=",
                Key::ShowLetters => "ABC",
                Key::Character(' ') => "Space",
                Key::Character(c) => c.encode_utf8(&mut buffer),
            };
            Text::with_text_style(
                label,
                bounds.center(),
                style.text(TextRole::Body),
                TextStyleBuilder::new()
                    .alignment(Alignment::Center)
                    .baseline(Baseline::Middle)
                    .build(),
            )
            .draw(&mut target)?;
        }
        Ok(())
    }
}
