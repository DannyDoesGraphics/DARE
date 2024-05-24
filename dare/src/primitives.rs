use dagal::allocators::{GPUAllocatorImpl, VkMemAllocator};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::Destructible;
use std::mem;
use dagal::descriptor::bindless::bindless::GPUResourceTableHandle;
use dagal::descriptor::GPUResourceTable;

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub uv_x: f32,
    pub normal: glam::Vec3,
    pub uv_y: f32,
    pub color: glam::Vec4,
}

#[derive(Clone)]
pub struct GPUMeshBuffer {
    pub index_buffer: dagal::resource::Buffer<u32, GPUAllocatorImpl>,
    pub vertex_buffer: GPUResourceTableHandle<dagal::resource::Buffer<u8, GPUAllocatorImpl>>,
}

impl GPUMeshBuffer {
    pub fn new(
        allocator: &mut dagal::allocators::SlotMapMemoryAllocator<GPUAllocatorImpl>,
        immediate: &mut dagal::util::ImmediateSubmit,
        gpu_resource_table: &mut GPUResourceTable,
        indices: &[u32],
        vertices: &[Vertex],
        name: Option<String>,
    ) -> Self {
        let mut index_buffer = dagal::resource::Buffer::<u32, GPUAllocatorImpl>::new(
            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: immediate.get_device().clone(),
                allocator,
                size: indices.len() as u64,
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::INDEX_BUFFER,
            },
        )
        .unwrap();
        let mut vertex_buffer_handle = gpu_resource_table.new_buffer(
            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: immediate.get_device().clone(),
                allocator,
                size: mem::size_of_val(vertices) as vk::DeviceSize,
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            }
        ).unwrap();
        let mut vertex_buffer = gpu_resource_table.get_buffer(&vertex_buffer_handle).unwrap();
        index_buffer.upload(immediate, allocator, indices).unwrap(); // fuck it lol
        unsafe {
            vertex_buffer
                .upload_arbitrary(immediate, allocator, vertices)
                .unwrap();
        }
        if let Some(debug_utils) = immediate.get_device().get_debug_utils() {
            if let Some(name) = name {
                let vertex_name = {
                    let mut n = name.clone();
                    n.push_str(" vertex");
                    n
                };
                let index_name = {
                    let mut n = name.clone();
                    n.push_str(" index");
                    n
                };
                index_buffer.set_name(debug_utils, index_name.as_str()).unwrap();
                vertex_buffer.set_name(debug_utils, vertex_name.as_str()).unwrap()
            }
        }
        Self {
            index_buffer,
            vertex_buffer: vertex_buffer_handle,
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
    pub vertex_buffer_id: u32,
}

#[derive(Debug, Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct GeometrySurface {
    pub start_index: u32,
    pub count: u32,
}

#[derive(Clone)]
pub struct MeshAsset {
    pub name: String,

    pub surfaces: Vec<GeometrySurface>,
    pub mesh_buffers: GPUMeshBuffer,
}