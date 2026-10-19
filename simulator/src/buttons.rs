//! Hardware controls for the native board profile.

use paper_mesh_platform::input::buttons::{
    ButtonCapability, ButtonDebouncer, ButtonDescriptor, ButtonEvent, ButtonId,
};
use serde::Serialize;

pub const FUNCTION: ButtonId = ButtonId(1);
pub const HOME: ButtonId = ButtonId(2);
const DESCRIPTORS: [ButtonDescriptor; 5] = [
    ButtonDescriptor {
        id: ButtonId(0),
        label: "BOOT",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: FUNCTION,
        label: "Function",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: HOME,
        label: "Home",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: ButtonId(3),
        label: "Reset",
        capability: ButtonCapability::Reset,
    },
    ButtonDescriptor {
        id: ButtonId(4),
        label: "QON",
        capability: ButtonCapability::PowerRecovery,
    },
];

#[derive(Serialize)]
pub struct ButtonSnapshot {
    pub id: u8,
    pub label: &'static str,
    pub capability: &'static str,
    pub pressed: bool,
}

pub struct SimButtons {
    states: [ButtonDebouncer; 3],
    pressed: [bool; 3],
}

impl Default for SimButtons {
    fn default() -> Self {
        let mut states = core::array::from_fn(|id| ButtonDebouncer::new(ButtonId(id as u8), 0));
        for state in &mut states {
            state.sample(false, 0);
        }
        Self {
            states,
            pressed: [false; 3],
        }
    }
}

impl SimButtons {
    /// The API supplies debounced states. Lifecycle controls have no input events.
    pub fn sample(
        &mut self,
        id: ButtonId,
        pressed: bool,
        at_ms: u64,
    ) -> Result<Option<ButtonEvent>, &'static str> {
        let state = self
            .states
            .get_mut(usize::from(id.0))
            .ok_or("This control has no readable button signal.")?;
        self.pressed[usize::from(id.0)] = pressed;
        Ok(state.sample(pressed, at_ms))
    }

    pub fn snapshot(&self) -> Vec<ButtonSnapshot> {
        DESCRIPTORS
            .iter()
            .map(|d| ButtonSnapshot {
                id: d.id.0,
                label: d.label,
                capability: match d.capability {
                    ButtonCapability::Edges => "edges",
                    ButtonCapability::Activation => "activation",
                    ButtonCapability::Reset => "reset",
                    ButtonCapability::PowerRecovery => "power_recovery",
                },
                pressed: self
                    .pressed
                    .get(usize::from(d.id.0))
                    .copied()
                    .unwrap_or(false),
            })
            .collect()
    }
}
