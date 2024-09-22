use dagal::allocators::Allocator;
use bevy_ecs::prelude as becs;
use crate::asset::prelude as asset;



/// Creates a mesh
#[derive(becs::Bundle)]
pub struct Mesh<A: Allocator + 'static> {
    /// Reference to the underlying surface
    pub surface: asset::Surface<A>,
}