use dagal::allocators::{Allocator, GPUAllocatorImpl};

use crate::util::handle;

/// Describes a surface which can be rendered
#[derive(Debug, Clone)]
pub struct Surface<A: Allocator = GPUAllocatorImpl> {
    vertex_buffer: handle::BufferHandle<A>,
    index_buffer: handle::BufferHandle<A>,
    normal_buffer: Option<handle::BufferHandle<A>>,
}