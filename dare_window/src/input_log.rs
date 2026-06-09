use bevy_ecs::prelude::*;
use dagal::winit;

use crate::input::Input;

/// Input recorded during winit events, drained by ECS systems on the next tick.
#[derive(Debug, Resource, Default)]
pub struct InputLog {
    events: Vec<Input>,
    modifiers: winit::keyboard::ModifiersState,
}

impl InputLog {
    pub fn push(&mut self, event: Input) {
        self.events.push(event);
    }

    pub fn set_modifiers(&mut self, modifiers: winit::keyboard::ModifiersState) {
        self.modifiers = modifiers;
    }

    pub fn modifiers(&self) -> winit::keyboard::ModifiersState {
        self.modifiers
    }

    pub fn drain(&mut self) -> Vec<Input> {
        std::mem::take(&mut self.events)
    }
}
