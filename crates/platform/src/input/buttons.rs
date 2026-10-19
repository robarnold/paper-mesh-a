//! Hardware control identities and debounced observations.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonCapability {
    Edges,
    Activation,
    Reset,
    PowerRecovery,
}

/// Identity and electrical capability do not select an application action.
#[derive(Clone, Copy, Debug)]
pub struct ButtonDescriptor {
    pub id: ButtonId,
    pub label: &'static str,
    pub capability: ButtonCapability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    /// Establishes initial state without an invented press or release.
    Synchronized {
        pressed: bool,
    },
    Pressed,
    Released,
    Activated,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonEvent {
    pub id: ButtonId,
    pub state: ButtonState,
    pub at_ms: u64,
}

/// The backend supplies a debounce interval and repeated electrical samples.
pub struct ButtonDebouncer {
    id: ButtonId,
    interval_ms: u64,
    stable: Option<bool>,
    candidate: Option<(bool, u64)>,
}

impl ButtonDebouncer {
    pub const fn new(id: ButtonId, interval_ms: u64) -> Self {
        Self {
            id,
            interval_ms,
            stable: None,
            candidate: None,
        }
    }

    pub fn sample(&mut self, pressed: bool, at_ms: u64) -> Option<ButtonEvent> {
        if self
            .candidate
            .is_none_or(|(value, since)| value != pressed || at_ms < since)
        {
            self.candidate = Some((pressed, at_ms));
        }
        let (_, since) = self.candidate?;
        if at_ms.saturating_sub(since) < self.interval_ms || self.stable == Some(pressed) {
            return None;
        }
        let state = match self.stable.replace(pressed) {
            None => ButtonState::Synchronized { pressed },
            Some(_) if pressed => ButtonState::Pressed,
            Some(_) => ButtonState::Released,
        };
        Some(ButtonEvent {
            id: self.id,
            state,
            at_ms,
        })
    }

    /// A fault invalidates the state. Recovery starts with synchronization.
    pub fn cancel(&mut self, at_ms: u64) -> Option<ButtonEvent> {
        self.candidate = None;
        self.stable.take().map(|_| ButtonEvent {
            id: self.id,
            state: ButtonState::Cancel,
            at_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_and_startup_holds_do_not_create_presses() {
        let mut button = ButtonDebouncer::new(ButtonId(7), 20);
        assert_eq!(button.sample(true, 0), None);
        assert_eq!(button.sample(true, 19), None);
        assert_eq!(
            button.sample(true, 20).unwrap().state,
            ButtonState::Synchronized { pressed: true }
        );
        assert_eq!(button.sample(false, 21), None);
        assert_eq!(button.sample(true, 30), None);
        assert_eq!(button.sample(false, 40), None);
        assert_eq!(
            button.sample(false, 60).unwrap().state,
            ButtonState::Released
        );
        assert_eq!(button.sample(true, 61), None);
        assert_eq!(button.sample(true, 81).unwrap().state, ButtonState::Pressed);
        assert_eq!(button.sample(true, 100), None);
    }

    #[test]
    fn failure_and_clock_reset_require_synchronization() {
        let mut button = ButtonDebouncer::new(ButtonId(2), 10);
        button.sample(false, 100);
        button.sample(false, 110);
        assert_eq!(button.cancel(111).unwrap().state, ButtonState::Cancel);
        assert_eq!(button.cancel(112), None);
        button.sample(true, 120);
        assert_eq!(button.sample(true, 5), None);
        assert_eq!(
            button.sample(true, 15).unwrap().state,
            ButtonState::Synchronized { pressed: true }
        );
    }
}
