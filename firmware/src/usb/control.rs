//! Bind reset requests and replies to one observed CDC connection.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub generation: u32,
    pub owner: u32,
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

    pub fn owner(&self, generation: u32) -> Option<u32> {
        (self.connected && self.generation == Some(generation)).then_some(self.epoch)
    }

    pub fn connected(&self, owner: u32, generation: u32) -> bool {
        self.owner(generation) == Some(owner)
    }

    pub fn reply_session_live(&self, generation: u32, owner: Option<u32>) -> bool {
        self.generation == Some(generation)
            && owner.is_none_or(|owner| self.connected(owner, generation))
    }

    pub fn begin_reply(&mut self, owner: u32, generation: u32) -> Option<Ticket> {
        if !self.connected(owner, generation) || self.live.is_some() {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let ticket = Ticket {
            generation,
            owner,
            sequence: self.sequence,
        };
        self.live = Some(ticket);
        Some(ticket)
    }

    pub fn reply_live(&self, ticket: Ticket) -> bool {
        self.live == Some(ticket) && self.connected(ticket.owner, ticket.generation)
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
