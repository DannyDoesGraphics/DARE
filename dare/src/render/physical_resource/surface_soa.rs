use dagal::allocators::Allocator;

use super::super::util::persistent_buffer::PersistentBuffer;
use crate::{engine::components::Surface, prelude::render::util::GrowableBuffer};
use dagal::ash::vk;

/// Struct of array manager for surfaces
#[derive(Debug)]
pub struct SurfaceSOA<A: Allocator> {
    pub visibility_buffer_mask: PersistentBuffer<A, u8>,
    pub transform_buffer_addresses: PersistentBuffer<A, u64>,
    pub minimum_buffer_addresses: PersistentBuffer<A, u64>,
    pub maximum_buffer_addresses: PersistentBuffer<A, u64>,
    pub material_buffer_addresses: PersistentBuffer<A, u64>,
    pub surface_flag_buffer_addresses: PersistentBuffer<A, u32>,
    pub index_count_buffer_addresses: PersistentBuffer<A, u64>,
}

impl<A: Allocator> SurfaceSOA<A> {
    pub fn new(mut allocator: A, device: dagal::device::LogicalDevice) -> anyhow::Result<Self> {
        let usage_flags: vk::BufferUsageFlags = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_DST
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        Ok(Self {
            visibility_buffer_mask: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::exists_buffer_mask".to_string()),
                    allocator: &mut allocator,
                    size: 1024,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            transform_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::transform_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 4 * 16,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            minimum_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::minimum_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 8,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            maximum_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::maximum_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 8,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            material_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::material_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 8,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            surface_flag_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::surface_flag_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 8,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
            index_count_buffer_addresses: PersistentBuffer::new(GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some("SurfaceSOA::index_count_buffer_addresses".to_string()),
                    allocator: &mut allocator,
                    size: 1024 * 8 * 8,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags,
                },
            )?),
        })
    }
}
