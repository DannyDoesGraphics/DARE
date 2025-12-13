use bevy_ecs::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Component)]
pub struct Velocity(pub glam::Vec3);

impl Eq for Velocity {}
