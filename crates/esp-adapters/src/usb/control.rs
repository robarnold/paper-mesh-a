//! Bind reset requests and replies to one observed CDC connection.
//!
//! # Design choices
//!
//! - Invalidate a pending reset reply on disconnect so a different host session
//!   cannot acknowledge it.
//!
//! See [Reply transport](../../../../docs/services/connectivity.md#reply-transport).

use paper_mesh_platform::transport::SessionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub generation: u32,
    pub session: SessionId,
    sequence: u32,
}

pub struct State {
    generation: Option<u32>,
    epoch: u32,
    connected: bool,
    sequence: u32,
    live: Option<Ticket>,
}

impl State {
    pub const fn new() -> Self {
        Self {
            generation: None,
            epoch: 0,
            connected: false,
            sequence: 0,
            live: None,
        }
    }

    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.connected = false;
        self.live = None;
    }

    pub fn observe(&mut self, generation: u32, connected: bool) {
        if self.generation != Some(generation) {
            self.invalidate();
            self.generation = Some(generation);
        }
        if self.connected != connected {
            self.invalidate();
            self.connected = connected;
        }
    }

    pub fn disconnect(&mut self, generation: u32) {
        if self.generation == Some(generation) && self.connected {
            self.invalidate();
        }
    }

    pub fn session(&self, generation: u32) -> Option<SessionId> {
        (self.connected && self.generation == Some(generation))
            .then_some(SessionId::new(self.epoch))
    }

    pub fn is_session_active(&self, session: SessionId, generation: u32) -> bool {
        self.session(generation) == Some(session)
    }

    pub fn reply_session_live(&self, generation: u32, session: Option<SessionId>) -> bool {
        self.generation == Some(generation)
            && session.is_none_or(|session| self.is_session_active(session, generation))
    }

    pub fn begin_reply(&mut self, session: SessionId, generation: u32) -> Option<Ticket> {
        if !self.is_session_active(session, generation) || self.live.is_some() {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let ticket = Ticket {
            generation,
            session,
            sequence: self.sequence,
        };
        self.live = Some(ticket);
        Some(ticket)
    }

    pub fn reply_live(&self, ticket: Ticket) -> bool {
        self.live == Some(ticket) && self.is_session_active(ticket.session, ticket.generation)
    }

    pub fn accepts_ack(&self, expected: Ticket, completed: Ticket, sent: bool) -> bool {
        expected == completed && sent && self.reply_live(expected)
    }

    pub fn cancel_reply(&mut self, ticket: Ticket) {
        if self.live == Some(ticket) {
            self.live = None;
        }
    }
}

#[cfg(test)]
mod tests;
