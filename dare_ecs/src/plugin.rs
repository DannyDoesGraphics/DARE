use bevy_ecs::prelude::*;

/// A plugin is initialized via [`Self::build`] which injects it into the application
pub trait Plugin {
    /// Initialization to inject plugin into world
    fn build(&self, world: &mut super::App);
}