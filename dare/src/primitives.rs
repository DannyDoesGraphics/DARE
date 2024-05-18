use dagal::allocators::{Allocator, VkMemAllocator};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::Destructible;
use std::mem;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub uv_x: f32,
    pub normal: glam::Vec3,
    pub uv_y: f32,
    pub color: glam::Vec4,
}

#[derive(Clone)]
pub struct GPUMeshBuffer {
    pub index_buffer: dagal::resource::Buffer<u32, VkMemAllocator>,
    pub vertex_buffer: dagal::resource::Buffer<Vertex, VkMemAllocator>,
}

impl GPUMeshBuffer {
    pub fn new(
        allocator: &mut dagal::allocators::SlotMapMemoryAllocator<VkMemAllocator>,
        immediate: &mut dagal::util::ImmediateSubmit,
        indices: &[u32],
        vertices: &[Vertex],
    ) -> Self {
        let mut index_buffer = dagal::resource::Buffer::<u32, VkMemAllocator>::new(
            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: immediate.get_device().clone(),
                allocator,
                size: mem::size_of_val(indices) as u64,
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::INDEX_BUFFER,
            },
        )
        .unwrap();
        let mut vertex_buffer = dagal::resource::Buffer::<Vertex, VkMemAllocator>::new(
            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: immediate.get_device().clone(),
                allocator,
                size: mem::size_of_val(vertices) as u64,
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            },
        )
        .unwrap();
        index_buffer.upload(immediate, allocator, indices).unwrap(); // fuck it lol
        vertex_buffer
            .upload(immediate, allocator, vertices)
            .unwrap();

        Self {
            index_buffer,
            vertex_buffer,
        }
    }
}

impl Destructible for GPUMeshBuffer {
    fn destroy(&mut self) {
        self.index_buffer.destroy();
        self.vertex_buffer.destroy();
    }
}

#[repr(C)]
pub struct GPUDrawPushConstants {
    pub world_matrix: glam::Mat4,
    pub vertex_buffer: vk::DeviceAddress,
}
