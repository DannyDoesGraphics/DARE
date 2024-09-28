use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;

/// Creates a mesh
#[derive(becs::Bundle)]
pub struct Mesh<A: Allocator + 'static> {
    /// Reference to the underlying surface
    pub surface: dare::asset::SurfaceMetadata<A>,
    /// Expect a transform
    pub transform: dare::physics::components::Transform,
}
