use super::prelude as asset;
use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;
use bevy_ecs::prelude as becs;

/// A surface directly contains references to the underlying data it is supposed to represent
#[derive(Debug, Clone, becs::Component)]
pub struct WeakSurface<A: Allocator + 'static> {
    pub vertex_buffer: containers::WeakDeferredDeletionSlot<resource::Buffer<A>>,
    pub index_buffer: Option<containers::WeakDeferredDeletionSlot<resource::Buffer<A>>>,
    pub normal_buffer: Option<containers::WeakDeferredDeletionSlot<resource::Buffer<A>>>,
    pub tangent_buffer: Option<containers::WeakDeferredDeletionSlot<resource::Buffer<A>>>,
    pub uv_buffers: Vec<containers::WeakDeferredDeletionSlot<resource::Buffer<A>>>,
    pub texture: Option<asset::WeakTexture<A>>,
}

impl<A: Allocator> WeakSurface<A> {

    /// We expect all data from [`WeakSurface`] to also exist in [`StrongSurface`]
    pub fn upgrade_strict(&self) -> Option<StrongSurface<A>> {
        Some(StrongSurface {
            vertex_buffer: self.vertex_buffer.upgrade()?,
            index_buffer: self.index_buffer.as_ref().and_then(|ib| Some(ib.upgrade()?)),
            normal_buffer: self.normal_buffer.as_ref().and_then(|ib| Some(ib.upgrade()?)),
            tangent_buffer: self.tangent_buffer.as_ref().and_then(|ib| Some(ib.upgrade()?)),
            uv_buffers: self.uv_buffers.iter().map(|b| b.upgrade()).collect::<Option<Vec<_>>>()?,
            texture: self.texture.as_ref().and_then(|t| Some(t.upgrade()?)),
        })
    }

}

/// A surface directly contains references to the underlying data it is supposed to represent
#[derive(Debug, Clone)]
pub struct StrongSurface<A: Allocator + 'static> {
    pub vertex_buffer: containers::StrongDeferredDeletionSlot<resource::Buffer<A>>,
    pub index_buffer: Option<containers::StrongDeferredDeletionSlot<resource::Buffer<A>>>,
    pub normal_buffer: Option<containers::StrongDeferredDeletionSlot<resource::Buffer<A>>>,
    pub tangent_buffer: Option<containers::StrongDeferredDeletionSlot<resource::Buffer<A>>>,
    pub uv_buffers: Vec<containers::StrongDeferredDeletionSlot<resource::Buffer<A>>>,
    pub texture: Option<asset::StrongTexture<A>>,
}
