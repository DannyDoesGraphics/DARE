use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dagal::resource::traits::Resource;

/// Contains all information relating to surfaces
#[derive(Debug)]
pub struct SurfacesContext {
    /// The n buffers containing surface information where `n` is the number of frames in flight
    pub surface_buffers: Vec<dagal::resource::Buffer<GPUAllocatorImpl>>,
}

impl SurfacesContext {
    pub fn new(
        frame_in_flight: u32,
        device: &dagal::device::LogicalDevice,
        allocator: &mut GPUAllocatorImpl,
    ) -> Result<Self, dagal::DagalError> {
        let surface_buffers = (0..frame_in_flight)
            .map(|index| {
                dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: Some(format!("surface_information_buffer_{index}")),
                    allocator,
                    // assume 256 surfaces for now, we will expand this later
                    size: vk::DeviceSize::from(
                        std::mem::size_of::<crate::render::c::CSurface>() as u64 * 256u64,
                    ),
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::TRANSFER_SRC
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                })
            })
            .collect::<Result<Vec<dagal::resource::Buffer<GPUAllocatorImpl>>, _>>()?;
        Ok(Self { surface_buffers })
    }
}
