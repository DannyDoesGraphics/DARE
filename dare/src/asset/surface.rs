use super::prelude as asset;
use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;
use bevy_ecs::prelude as becs;

/// A surface directly contains references to the underlying data it is supposed to represent
#[derive(Debug, Clone, becs::Component)]
pub struct Surface<A: Allocator + 'static> {
    pub vertex_buffer: asset::BufferMetaData<A>,
    pub index_buffer: Option<asset::BufferMetaData<A>>,
    pub normal_buffer: Option<asset::BufferMetaData<A>>,
    pub tangent_buffer: Option<asset::BufferMetaData<A>>,
    pub uv_buffers: Vec<asset::BufferMetaData<A>>,
    pub texture: Option<asset::BufferMetaData<A>>,
}
