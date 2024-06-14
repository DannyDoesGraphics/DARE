use std::sync::Arc;

use dagal::allocators::GPUAllocatorImpl;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

use crate::primitives::MaterialInstance;

pub struct RenderObject {
    pub index_count: u32,
    pub first_index: u32,

    pub material: Arc<MaterialInstance>,
    pub transform: glam::Mat4,
    pub vertex_buffer: Handle<resource::Buffer<GPUAllocatorImpl>>,
    pub index_buffer: Handle<resource::Buffer<GPUAllocatorImpl>>,
}
