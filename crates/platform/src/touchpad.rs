//! Contact forwarding to an optional host without application dependencies.

use crate::input::contacts::{ContactState, TouchFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Disconnected,
    Ready,
    Suspended,
    Fault,
}

#[derive(Clone, Copy, Debug)]
pub struct HostFrame {
    pub contacts: ContactState,
    pub at_ms: u64,
}

pub trait TouchpadHost {
    fn state(&self) -> HostState;
    /// Changes after every transport reset, including resets between polls.
    fn generation(&self) -> u32;
    fn set_active(&mut self, active: bool);
    /// Cancels host gesture state. A backend can detach its device to guarantee cancellation.
    fn cancel(&mut self);
    /// Returns false when the backend cannot retain the complete frame.
    fn submit(&mut self, frame: HostFrame) -> bool;
}

pub struct TouchpadSession {
    active: bool,
    waiting_release: bool,
    contacts: ContactState,
    host_state: HostState,
    generation: u32,
}

impl Default for TouchpadSession {
    fn default() -> Self {
        Self::new()
    }
}
impl TouchpadSession {
    pub const fn new() -> Self {
        Self {
            active: false,
            waiting_release: false,
            contacts: ContactState::new(),
            host_state: HostState::Disconnected,
            generation: 0,
        }
    }
    pub fn set_active(&mut self, active: bool, host: &mut impl TouchpadHost) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.waiting_release = !self.contacts.is_empty();
        host.set_active(active);
    }
    pub fn poll(&mut self, host: &impl TouchpadHost) {
        let state = host.state();
        if state != self.host_state || self.generation != host.generation() {
            self.waiting_release |= self.active && !self.contacts.is_empty();
            self.host_state = state;
            self.generation = host.generation();
        }
    }
    /// Returns true only when ordinary application controls can consume this frame.
    pub fn input(&mut self, frame: &TouchFrame, host: &mut impl TouchpadHost) -> bool {
        self.poll(host);
        let captured = self.active || self.waiting_release;
        if !self.contacts.apply(frame) {
            if self.active {
                host.cancel();
            }
            self.waiting_release = false;
            return !captured;
        }
        if self.waiting_release {
            if self.contacts.is_empty() {
                self.waiting_release = false;
            }
            return false;
        }
        if self.active
            && self.host_state == HostState::Ready
            && !host.submit(HostFrame {
                contacts: self.contacts,
                at_ms: frame.at_ms,
            })
        {
            host.cancel();
            self.waiting_release = !self.contacts.is_empty();
        }
        !captured
    }
    pub fn contacts(&self) -> &ContactState {
        &self.contacts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TouchEvent;
    use embedded_graphics::prelude::Point;
    struct Host {
        active: bool,
        state: HostState,
        sent: usize,
        cancels: usize,
        accept: bool,
        generation: u32,
    }
    impl TouchpadHost for Host {
        fn generation(&self) -> u32 {
            self.generation
        }
        fn state(&self) -> HostState {
            self.state
        }
        fn set_active(&mut self, active: bool) {
            self.active = active;
            if !active {
                self.cancel();
            }
        }
        fn cancel(&mut self) {
            self.cancels += 1;
        }
        fn submit(&mut self, _: HostFrame) -> bool {
            self.sent += 1;
            self.accept
        }
    }
    fn frame(pressed: bool) -> TouchFrame {
        TouchFrame::single(
            if pressed {
                TouchEvent::touch(1, Point::new(10, 20))
            } else {
                TouchEvent::release(1, Point::new(10, 20))
            },
            10,
        )
    }
    #[test]
    fn mode_changes_and_reconnects_require_fresh_contacts() {
        let mut host = Host {
            active: false,
            state: HostState::Ready,
            sent: 0,
            cancels: 0,
            accept: true,
            generation: 0,
        };
        let mut session = TouchpadSession::new();
        assert!(session.input(&frame(true), &mut host));
        session.set_active(true, &mut host);
        assert!(!session.input(&frame(true), &mut host));
        session.input(&frame(false), &mut host);
        assert_eq!(host.sent, 0);
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 1);
        host.generation += 1;
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 1);
        session.input(&frame(false), &mut host);
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 2);
        host.state = HostState::Suspended;
        session.poll(&host);
        host.state = HostState::Ready;
        session.poll(&host);
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 2);
        session.input(&frame(false), &mut host);
        session.input(&frame(true), &mut host);
        session.set_active(false, &mut host);
        assert!(!session.input(&frame(false), &mut host));
        assert!(session.input(&frame(true), &mut host));
        assert_eq!(host.cancels, 1);
    }
    #[test]
    fn host_changes_do_not_capture_normal_application_input() {
        let mut host = Host {
            active: false,
            state: HostState::Ready,
            sent: 0,
            cancels: 0,
            accept: true,
            generation: 0,
        };
        let mut session = TouchpadSession::new();
        assert!(session.input(&frame(true), &mut host));
        host.generation += 1;
        host.state = HostState::Disconnected;
        assert!(session.input(&frame(false), &mut host));
        assert_eq!(host.sent, 0);
    }
    #[test]
    fn backpressure_cancels_the_session_until_release() {
        let mut host = Host {
            active: false,
            state: HostState::Ready,
            sent: 0,
            cancels: 0,
            accept: false,
            generation: 0,
        };
        let mut session = TouchpadSession::new();
        session.set_active(true, &mut host);
        session.input(&frame(true), &mut host);
        assert_eq!(host.cancels, 1);
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 1);
        session.input(&frame(false), &mut host);
        host.accept = true;
        session.input(&frame(true), &mut host);
        assert_eq!(host.sent, 2);
    }
}
