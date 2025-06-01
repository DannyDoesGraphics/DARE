use crate::allocators::Allocator;
use ash::vk;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MemoryBarrier {
    src_stage_mask: vk::PipelineStageFlags2,
    src_access_mask: vk::AccessFlags2,
    dst_stage_mask: vk::PipelineStageFlags2,
    dst_access_mask: vk::AccessFlags2,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BufferMemoryBarrier<'a, A: Allocator> {
    memory_barrier: MemoryBarrier,
    src_queue_family_index: u32,
    dst_queue_family_index: u32,
    buffer: &'a crate::resource::Buffer<A>,
    offset: vk::DeviceSize,
    size: vk::DeviceSize,
}

#[derive(Debug, Clone)]
pub struct DependencyInfo<'a, A: Allocator> {
    pub(crate) dependency_flags: vk::DependencyFlags,
    pub(crate) memory_barriers: Vec<MemoryBarrier>,
    pub(crate) buffer_memory_barriers: Vec<BufferMemoryBarrier<'a, A>>,
}

impl<A: Allocator> Default for DependencyInfo<'_, A> {
    fn default() -> Self {
        Self {
            dependency_flags: vk::DependencyFlags::empty(),
            memory_barriers: Vec::new(),
            buffer_memory_barriers: Vec::new(),
        }
    }
}
