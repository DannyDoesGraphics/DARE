use bevy_ecs::prelude::*;

#[derive(Debug, Component, PartialEq)]
pub struct Camera {
    pub fov: f32,
    pub transform: glam::Mat4,
}
