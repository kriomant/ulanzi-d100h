use std::collections::BTreeSet;
use crate::{Button, Direction, Event};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Button { button: Button, pressed: bool, held: BTreeSet<Button> },
    KnobClick { held: BTreeSet<Button> },
    Rotation { direction: Direction, knob_pressed: bool, held: BTreeSet<Button> },
}

/// Tracks physical buttons, excluding the knob's synthetic click pair.
#[derive(Clone, Debug, Default)]
pub struct ButtonState { held: BTreeSet<Button> }

impl ButtonState {
    pub fn held(&self) -> &BTreeSet<Button> { &self.held }
    /// Call on disconnect or loss of event continuity.
    pub fn clear(&mut self) { self.held.clear(); }
    /// Returned button snapshots represent the state after applying the transition.
    pub fn apply(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Button { button, pressed } => {
                if pressed { self.held.insert(button); } else { self.held.remove(&button); }
                Some(Action::Button { button, pressed, held: self.held.clone() })
            }
            Event::KnobClick { pressed: true } => Some(Action::KnobClick { held: self.held.clone() }),
            Event::KnobClick { pressed: false } => None,
            Event::Rotation { direction, knob_pressed } => Some(Action::Rotation {
                direction, knob_pressed, held: self.held.clone(),
            }),
        }
    }
}
