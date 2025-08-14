use ash::vk;

/// Contains memory state
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct MemoryState {
    stage_mask: vk::PipelineStageFlags2,
    access_mask: vk::AccessFlags,
}
