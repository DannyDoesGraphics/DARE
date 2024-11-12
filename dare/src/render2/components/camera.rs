use crate::prelude as dare;
use bevy_ecs::prelude as becs;

#[derive(Debug, PartialEq, Copy, Clone, becs::Component)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(becs::Bundle, Debug, Clone)]
pub struct CameraBundle {
    camera: Camera,
    transform: dare::physics::components::Transform,
}
