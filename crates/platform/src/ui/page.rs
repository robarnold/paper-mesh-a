//! Content and lifecycle contract for one application destination.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

use crate::TouchEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Standard,
    FullScreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation<Id> {
    Home,
    Back,
    Open(Id),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageEvent<Id> {
    None,
    Changed,
    Navigate(Navigation<Id>),
}

impl<Id> PageEvent<Id> {
    pub fn changed(changed: bool) -> Self {
        if changed { Self::Changed } else { Self::None }
    }
}

#[derive(Clone, Copy)]
pub struct PageInfo<Id> {
    pub id: Id,
    pub title: &'static str,
    pub context: &'static str,
    pub presentation: Presentation,
    pub navigation_enabled: bool,
}

/// Applications own page storage and supply their destination identities.
pub trait Page {
    type Id: Copy + Eq;

    fn page_info(&self) -> PageInfo<Self::Id>;
    fn enter(&mut self);
    fn leave(&mut self);
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Self::Id>;
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error>;
}

/// Dispatch to retained pages with application-defined identities and storage.
pub trait PageSet {
    type Id: Copy + Eq;

    fn info(&self, id: Self::Id) -> PageInfo<Self::Id>;
    fn enter_page(&mut self, id: Self::Id);
    fn leave_page(&mut self, id: Self::Id);
    fn touch_page(&mut self, id: Self::Id, event: TouchEvent) -> PageEvent<Self::Id>;
    fn draw_page<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Self::Id,
        display: &mut D,
    ) -> Result<(), D::Error>;
}

impl<P: Page> PageSet for P {
    type Id = P::Id;

    fn info(&self, id: Self::Id) -> PageInfo<Self::Id> {
        let info = Page::page_info(self);
        assert!(info.id == id, "page identity must match the destination");
        info
    }

    fn enter_page(&mut self, _: Self::Id) {
        Page::enter(self);
    }

    fn leave_page(&mut self, _: Self::Id) {
        Page::leave(self);
    }

    fn touch_page(&mut self, _: Self::Id, event: TouchEvent) -> PageEvent<Self::Id> {
        Page::handle_touch(self, event)
    }

    fn draw_page<D: DrawTarget<Color = Gray4>>(
        &self,
        _: Self::Id,
        display: &mut D,
    ) -> Result<(), D::Error> {
        Page::render_page(self, display)
    }
}
