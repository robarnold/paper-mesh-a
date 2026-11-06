//! Page behavior and lifecycle for application-defined destinations.
//!
//! # Design choices
//!
//! - Let the application supply destination IDs and page storage so the toolkit can
//!   navigate without knowing which screens exist.
//! - Have pages request navigation through a return value so the navigator can apply
//!   restrictions before calling the exit and entry callbacks.
//!
//! See the [pages and shells contract](../../../../docs/platform/toolkit.md#pages-and-shells).

use embedded_graphics::{pixelcolor::Gray4, prelude::*};

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

/// Application callbacks receive this context after layer traversal.
pub struct UiContext<Id> {
    result: PageEvent<Id>,
    commands: heapless::Vec<(super::elements::ElementId, heapless::String<64>), 16>,
}
impl<Id> UiContext<Id> {
    fn new() -> Self {
        Self {
            result: PageEvent::None,
            commands: heapless::Vec::new(),
        }
    }
    pub fn respond(&mut self, result: PageEvent<Id>) {
        self.result = result;
    }
    pub fn changed(&mut self) {
        self.result = PageEvent::Changed;
    }
    pub fn navigate(&mut self, navigation: Navigation<Id>) {
        self.result = PageEvent::Navigate(navigation);
    }
    pub fn set_text(
        &mut self,
        element: super::elements::ElementId,
        text: &str,
    ) -> Result<(), super::elements::ElementError> {
        let text = heapless::String::try_from(text)
            .map_err(|_| super::elements::ElementError::TextCapacity)?;
        self.commands
            .push((element, text))
            .map_err(|_| super::elements::ElementError::Capacity)?;
        self.changed();
        Ok(())
    }
}

pub trait ElementSet<P: Page> {
    fn resolve(&mut self, style: &super::Style) -> Result<(), super::elements::ElementError>;
    fn publish(
        &mut self,
        tx: &mut crate::composition::Transaction<'_, 64>,
        root: crate::composition::LayerId,
        style: &super::Style,
    ) -> Result<(), crate::composition::Error>;
    fn dispatch(page: &mut P, slot: u16, gesture: crate::composition::Gesture) -> PageEvent<P::Id>;
    fn set_text(
        &mut self,
        element: super::elements::ElementId,
        text: &str,
    ) -> Result<(), super::elements::ElementError>;
    fn invalidate(&mut self);
}
impl<P: Page<Elements = Self>, A: Copy + Eq, const N: usize, const T: usize> ElementSet<P>
    for super::elements::Elements<P, A, N, T>
{
    fn resolve(&mut self, style: &super::Style) -> Result<(), super::elements::ElementError> {
        self.resolve(style)
    }
    fn publish(
        &mut self,
        tx: &mut crate::composition::Transaction<'_, 64>,
        root: crate::composition::LayerId,
        style: &super::Style,
    ) -> Result<(), crate::composition::Error> {
        self.publish(tx, root, style)
    }
    fn set_text(
        &mut self,
        id: super::elements::ElementId,
        text: &str,
    ) -> Result<(), super::elements::ElementError> {
        self.set_text(id, text)
    }
    fn invalidate(&mut self) {
        self.invalidate();
    }
    fn dispatch(page: &mut P, slot: u16, gesture: crate::composition::Gesture) -> PageEvent<P::Id> {
        let mut context = UiContext::new();
        match gesture {
            crate::composition::Gesture::Tap => {
                let Some((callback, data)) = page.elements().activation(slot) else {
                    return PageEvent::None;
                };
                callback(page, &mut context, data);
            }
            pan => {
                let Some((callback, data)) = page.elements().pan_activation(slot) else {
                    return PageEvent::None;
                };
                callback(page, &mut context, data, pan);
            }
        }
        for (id, text) in context.commands {
            page.elements_mut()
                .set_text(id, &text)
                .expect("callback text fits element");
        }
        context.result
    }
}

/// A page owns application state and an element composition, never a graphics/input store.
pub trait Page: Sized {
    type Id: Copy + Eq;
    type Elements: ElementSet<Self>;
    fn elements(&self) -> &Self::Elements;
    fn elements_mut(&mut self) -> &mut Self::Elements;
    fn page_info(&self) -> PageInfo<Self::Id>;
    fn did_enter(&mut self) {}
    fn will_leave(&mut self) {}
    fn sync_elements(&mut self) {}
    fn configure_handlers(&mut self) {}
    fn text_focus(&self) -> Option<super::TextFocus> {
        None
    }
    fn text_input(&mut self, _field: u16, _input: super::TextInput) -> bool {
        false
    }
    fn style(&self) -> super::Style {
        super::Style::GRAYSCALE
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _slot: u16,
        _target: &mut D,
    ) -> Result<(), D::Error> {
        Ok(())
    }
}
pub trait PageRuntime: Page {
    fn prepare(&mut self) -> Result<(), super::elements::ElementError> {
        self.configure_handlers();
        self.sync_elements();
        let style = self.style();
        self.elements_mut().resolve(&style)
    }
    fn enter(&mut self) {
        self.did_enter();
        self.elements_mut().invalidate();
        self.prepare().expect("page layout fits");
    }
    fn leave(&mut self) {
        self.will_leave();
    }
    fn activate(&mut self, slot: u16, gesture: crate::composition::Gesture) -> PageEvent<Self::Id> {
        Self::Elements::dispatch(self, slot, gesture)
    }
    fn publish(
        &mut self,
        tx: &mut crate::composition::Transaction<'_, 64>,
        root: crate::composition::LayerId,
    ) -> Result<(), crate::composition::Error> {
        self.prepare()
            .map_err(|_| crate::composition::Error::InvalidGeometry)?;
        let style = self.style();
        self.elements_mut().publish(tx, root, &style)
    }
    fn invalidate(&mut self) {
        self.elements_mut().invalidate();
    }
}
impl<P: Page> PageRuntime for P {}

pub trait PageSet {
    fn text_focus_page(&self, _id: Self::Id) -> Option<super::TextFocus> {
        None
    }
    fn text_input_page(&mut self, _id: Self::Id, _field: u16, _input: super::TextInput) -> bool {
        false
    }
    type Id: Copy + Eq;
    fn info(&self, id: Self::Id) -> PageInfo<Self::Id>;
    fn enter_page(&mut self, id: Self::Id);
    fn leave_page(&mut self, id: Self::Id);
    fn publish_page(
        &mut self,
        id: Self::Id,
        tx: &mut crate::composition::Transaction<'_, 64>,
        root: crate::composition::LayerId,
    ) -> Result<(), crate::composition::Error>;
    fn activate_page(
        &mut self,
        id: Self::Id,
        slot: u16,
        gesture: crate::composition::Gesture,
    ) -> PageEvent<Self::Id>;
    fn draw_page_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Self::Id,
        slot: u16,
        target: &mut D,
    ) -> Result<(), D::Error>;
    fn invalidate_page(&mut self, id: Self::Id);
}
impl<P: Page> PageSet for P {
    type Id = P::Id;
    fn text_focus_page(&self, _: Self::Id) -> Option<super::TextFocus> {
        self.text_focus()
    }
    fn text_input_page(&mut self, _: Self::Id, field: u16, input: super::TextInput) -> bool {
        self.text_input(field, input)
    }
    fn info(&self, id: Self::Id) -> PageInfo<Self::Id> {
        let info = self.page_info();
        assert!(info.id == id, "page identity must match");
        info
    }
    fn enter_page(&mut self, _: Self::Id) {
        self.enter();
    }
    fn leave_page(&mut self, _: Self::Id) {
        self.leave();
    }
    fn publish_page(
        &mut self,
        _: Self::Id,
        tx: &mut crate::composition::Transaction<'_, 64>,
        root: crate::composition::LayerId,
    ) -> Result<(), crate::composition::Error> {
        self.publish(tx, root)
    }
    fn activate_page(
        &mut self,
        _: Self::Id,
        slot: u16,
        gesture: crate::composition::Gesture,
    ) -> PageEvent<Self::Id> {
        self.activate(slot, gesture)
    }
    fn draw_page_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: Self::Id,
        slot: u16,
        target: &mut D,
    ) -> Result<(), D::Error> {
        Page::draw_custom(self, slot, target)
    }
    fn invalidate_page(&mut self, _: Self::Id) {
        self.invalidate();
    }
}
