
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct BufferState {
    pub memory: super::memory::MemoryState,
    pub usage: ash::vk::BufferUsageFlags,
}