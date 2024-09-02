use super::prelude as asset;
use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;

/// A surface directly contains references to the underlying data it is supposed to represent
///
///
#[derive(Debug, Clone)]
pub struct Surface<A: Allocator> {
    pub vertices: containers::DeferredDeletionSlot<resource::Buffer<A>>,
    pub texture: asset::Texture<A>,
}
