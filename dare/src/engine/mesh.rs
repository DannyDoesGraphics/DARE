use crate::asset::prelude as asset;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;

/// Creates a mesh
#[derive(becs::Bundle)]
pub struct Mesh<A: Allocator + 'static> {
    /// Reference to the underlying surface
    pub surface: asset::Surface<A>,
}
