use crate::prelude as dare;
use bevy_ecs::prelude as becs;

#[derive(becs::Bundle, Clone, Debug)]
pub struct Mesh {
    pub surface: super::Surface,
    pub bounding_box: dare::render::components::bounding_box::BoundingBox,
    pub name: dare::engine::components::Name,
    pub transform: dare::physics::components::Transform,
}
