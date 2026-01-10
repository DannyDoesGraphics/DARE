use bevy_ecs::prelude::*;

#[derive(Debug, PartialEq, Clone, Component)]
pub struct Camera {
    pitch: f32,
    yaw: f32,
    roll: f32,
}
