use bevy_ecs::prelude::Component;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Component)]
pub struct Name {
    pub name: String,
}