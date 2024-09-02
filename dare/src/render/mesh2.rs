use dagal::allocators::Allocator;
use dagal::resource;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
struct Surface<A: Allocator> {
    pub vertex_buffer: Arc<RwLock<resource::Buffer<A>>>,
    pub index_buffer: Arc<RwLock<resource::Buffer<A>>>,
    pub normal_buffer: Option<Arc<RwLock<resource::Buffer<A>>>>,
    pub uv_buffer: Option<Arc<RwLock<resource::Buffer<A>>>>,
    pub acceleration_structure: Option<Arc<RwLock<resource::AccelerationStructure>>>,
}
