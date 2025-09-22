use ash::vk;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Description {
    pub name: String,
    pub size: u64,
    pub usage: vk::BufferUsageFlags,
    pub transient: bool,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct State {
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
    pub queue_family: u32,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum AccessFlag {
    UniformBuffer,
    StorageBuffer,
    VertexBufferRead,
    IndexBufferRead,
    IndirectBufferRead,
}