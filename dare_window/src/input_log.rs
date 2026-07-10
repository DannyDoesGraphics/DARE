use bevy_ecs::prelude::*;
use dagal::winit;
use std::collections::HashSet;

use crate::input::Input;

/// Input recorded during winit events, read by ECS systems on the next tick.
///
/// `events` are per-frame and cleared each tick by [`Self::clear`]
#[derive(Debug, Resource, Default)]
pub struct InputLog {
    events: Vec<Input>,
    modifiers: winit::keyboard::ModifiersState,
    pressed_keys: HashSet<winit::keyboard::KeyCode>,
    pressed_buttons: HashSet<winit::event::MouseButton>,
}

impl InputLog {
    pub fn push(&mut self, event: Input) {
        match &event {
            Input::KeyEvent { event: key, .. } => {
                if let winit::keyboard::PhysicalKey::Code(code) = key.physical_key {
                    if key.state.is_pressed() {
                        self.pressed_keys.insert(code);
                    } else {
                        self.pressed_keys.remove(&code);
                    }
                }
            }
            Input::MouseButton { button, state } => {
                if state.is_pressed() {
                    self.pressed_buttons.insert(*button);
                } else {
                    self.pressed_buttons.remove(button);
                }
            }
            _ => {}
        }
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

    /// Non-destructive read of the events buffered this frame.
    pub fn events(&self) -> &[Input] {
        &self.events
    }

    /// Whether a key is currently held down.
    pub fn is_key_pressed(&self, code: winit::keyboard::KeyCode) -> bool {
        self.pressed_keys.contains(&code)
    }

    /// Whether a mouse button is currently held down.
    pub fn is_mouse_pressed(&self, button: winit::event::MouseButton) -> bool {
        self.pressed_buttons.contains(&button)
    }

    /// Drops all held key and button state.
    pub fn release_all(&mut self) {
        self.pressed_keys.clear();
        self.pressed_buttons.clear();
    }

    /// Clears the buffered events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}
