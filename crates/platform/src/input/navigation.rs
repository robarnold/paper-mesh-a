//! System navigation takes priority over application button bindings.

use super::buttons::{ButtonEvent, ButtonId, ButtonState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRoute {
    Home,
    Application(ButtonEvent),
    Consumed,
}

/// The backend identifies the key reserved for system navigation.
pub struct NavigationBindings {
    pub home: Option<ButtonId>,
}

impl NavigationBindings {
    /// Consume every Home-key event so an application cannot reuse the key.
    pub fn route(&self, event: ButtonEvent) -> ButtonRoute {
        if self.home != Some(event.id) {
            return ButtonRoute::Application(event);
        }
        match event.state {
            ButtonState::Pressed | ButtonState::Activated => ButtonRoute::Home,
            _ => ButtonRoute::Consumed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_reserved_in_all_states_without_invented_activation() {
        let bindings = NavigationBindings {
            home: Some(ButtonId(7)),
        };
        for state in [
            ButtonState::Synchronized { pressed: true },
            ButtonState::Synchronized { pressed: false },
            ButtonState::Pressed,
            ButtonState::Released,
            ButtonState::Activated,
            ButtonState::Cancel,
        ] {
            let event = ButtonEvent {
                id: ButtonId(7),
                state,
                at_ms: 42,
            };
            let expected = if matches!(state, ButtonState::Pressed | ButtonState::Activated) {
                ButtonRoute::Home
            } else {
                ButtonRoute::Consumed
            };
            assert_eq!(bindings.route(event), expected);
            let other = ButtonEvent {
                id: ButtonId(8),
                ..event
            };
            assert_eq!(bindings.route(other), ButtonRoute::Application(other));
            assert_eq!(
                NavigationBindings { home: None }.route(event),
                ButtonRoute::Application(event)
            );
        }
    }
}
