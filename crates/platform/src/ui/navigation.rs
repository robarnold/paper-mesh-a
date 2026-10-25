//! Shell launch, caller-based navigation, and page lifecycle dispatch.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use heapless::Vec;

use crate::TouchEvent;

use super::{Button, Navigation, PageEvent, PageInfo, PageSet, Shell, Style};

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition<Id> {
    pub from: Id,
    pub to: Id,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationError {
    HistoryFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update<Id> {
    pub changed: bool,
    pub transition: Option<Transition<Id>>,
}

impl<Id> Update<Id> {
    const NONE: Self = Self {
        changed: false,
        transition: None,
    };
}

/// One launched shell with retained pages and a bounded stack of return destinations.
pub struct Navigator<P: PageSet, const CAPACITY: usize> {
    shell: Shell,
    pages: P,
    home: P::Id,
    current: P::Id,
    history: Vec<P::Id, CAPACITY>,
    contacts: u16,
    suppressed: bool,
}

impl<P: PageSet, const CAPACITY: usize> Navigator<P, CAPACITY> {
    /// Enter the root page once before input or rendering can reach the shell.
    pub fn launch(home: P::Id, shell: Shell, mut pages: P) -> Self {
        assert!(pages.info(home).id == home, "root page identity must match");
        pages.enter_page(home);
        Self {
            shell,
            pages,
            home,
            current: home,
            history: Vec::new(),
            contacts: 0,
            suppressed: false,
        }
    }

    pub fn current(&self) -> P::Id {
        self.current
    }

    pub fn return_to(&self) -> Option<P::Id> {
        self.history.last().copied()
    }

    /// Return all callers, oldest first, without the current page.
    /// The last entry is the next Back destination.
    pub fn history(&self) -> &[P::Id] {
        self.history.as_slice()
    }

    pub fn pages(&self) -> &P {
        &self.pages
    }

    pub fn pages_mut(&mut self) -> &mut P {
        &mut self.pages
    }

    pub fn page_info(&self) -> PageInfo<P::Id> {
        let info = self.pages.info(self.current);
        assert!(info.id == self.current, "active page identity must match");
        info
    }

    pub fn back_control(&self) -> Option<Button<'static>> {
        self.shell
            .back_control(self.page_info(), self.return_to().is_some())
    }

    /// Rejected requests leave the destination, history, and lifecycle unchanged.
    pub fn navigate(
        &mut self,
        navigation: Navigation<P::Id>,
    ) -> Result<Option<Transition<P::Id>>, NavigationError> {
        if !self.page_info().navigation_enabled {
            return Ok(None);
        }
        let to = match navigation {
            Navigation::Home => self.home,
            Navigation::Back => match self.return_to() {
                Some(id) => id,
                None => return Ok(None),
            },
            Navigation::Open(id) => id,
        };
        if to == self.current {
            self.cancel_input();
            return Ok(None);
        }
        assert!(
            self.pages.info(to).id == to,
            "destination identity must match"
        );
        if to == self.home {
            self.history.clear();
        } else if navigation == Navigation::Back {
            self.history.pop();
        } else {
            self.history
                .push(self.current)
                .map_err(|_| NavigationError::HistoryFull)?;
        }
        self.cancel_input();
        let from = self.current;
        self.pages.leave_page(from);
        self.current = to;
        self.pages.enter_page(to);
        Ok(Some(Transition { from, to }))
    }

    fn cancel_input(&mut self) {
        self.shell.cancel();
        self.pages.touch_page(self.current, TouchEvent::Cancel);
        self.suppressed = self.contacts != 0;
    }

    /// Route input through the shared frame, then the active page.
    pub fn touch(&mut self, event: TouchEvent) -> Result<Update<P::Id>, NavigationError> {
        match event {
            TouchEvent::Touch(contact) | TouchEvent::Release(contact) => {
                let Some(bit) = 1_u16.checked_shl(u32::from(contact.id)) else {
                    self.cancel_input();
                    return Ok(Update::NONE);
                };
                if matches!(event, TouchEvent::Touch(_)) {
                    self.contacts |= bit;
                } else {
                    self.contacts &= !bit;
                }
                if self.suppressed {
                    self.suppressed = self.contacts != 0;
                    return Ok(Update::NONE);
                }
            }
            TouchEvent::Cancel => {
                self.contacts = 0;
                self.suppressed = false;
            }
        }
        let event = if self
            .shell
            .touch(event, self.page_info(), self.return_to().is_some())
        {
            PageEvent::Navigate(Navigation::Back)
        } else {
            self.pages.touch_page(self.current, event)
        };
        match event {
            PageEvent::None => Ok(Update::NONE),
            PageEvent::Changed => Ok(Update {
                changed: true,
                transition: None,
            }),
            PageEvent::Navigate(request) => {
                let transition = self.navigate(request)?;
                Ok(Update {
                    changed: transition.is_some(),
                    transition,
                })
            }
        }
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        self.shell.render(
            &self.pages,
            self.page_info(),
            self.return_to().is_some(),
            display,
            style,
        )
    }
}
