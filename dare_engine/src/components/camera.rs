pub use bevy_ecs::prelude::*;

#[derive(Debug, PartialEq, Clone, Component)]
pub struct Camera {
    pub fov: f64,
    pub yaw: f32,
    pub pitch: f32,
}
