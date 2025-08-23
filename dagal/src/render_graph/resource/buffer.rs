use crate::render_graph::resource::memory::MemoryState;
use crate::DefaultAllocator;
use ash::vk;

/// Describes buffer metadata
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BufferMetadata {
    name: String,
    size: vk::DeviceSize,
    usage_flags: vk::BufferUsageFlags,
    location: crate::allocators::MemoryLocation,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BufferState {
    pub metadata: MemoryState,
    pub queue_family_index: u32,
}

impl super::VirtualResourceMetadata for BufferMetadata {
    type PhysicalResource = crate::resource::Buffer<DefaultAllocator>;
    type PhysicalResourceState = BufferState;
}
