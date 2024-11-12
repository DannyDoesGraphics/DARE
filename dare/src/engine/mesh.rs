use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;

/// Creates a mesh
#[derive(becs::Bundle, Clone, Debug)]
pub struct Mesh {
    /// Reference to the underlying surface
    pub surface: dare::engine::components::Surface,
    /// Expect a transform
    pub transform: dare::physics::components::Transform,
}
