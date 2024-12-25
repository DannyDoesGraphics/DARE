use crate::prelude as dare;
use crate::render2::surface_context::SurfaceContext;
use anyhow::Result;
use dagal::allocators::{Allocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use std::collections::HashSet;
use std::fmt::Debug;
use std::ptr;
use std::sync::Arc;

/// Contains all information necessary to render current frame
#[derive(Debug)]
pub struct Frame {
    // Image that is being drawn to is here
    pub draw_image: dagal::resource::Image<GPUAllocatorImpl>,
    pub draw_image_view: dagal::resource::ImageView,
    pub depth_image: dagal::resource::Image<GPUAllocatorImpl>,
    pub depth_image_view: dagal::resource::ImageView,
    pub render_fence: dagal::sync::Fence,
    pub render_semaphore: dagal::sync::BinarySemaphore,
    pub swapchain_semaphore: dagal::sync::BinarySemaphore,
    pub queue: dagal::device::Queue,
    pub image_extent: vk::Extent2D,

    /// any resources binded for the current frame
    pub resources: HashSet<dare::asset2::AssetHandleUntyped>,
    /// Buffer used to hold indirect commands
    pub indirect_buffer: dare::render::util::GrowableBuffer<GPUAllocatorImpl>,
    /// Buffer used to hold instanced information
    pub instanced_buffer: dare::render::util::GrowableBuffer<GPUAllocatorImpl>,
    /// Buffer used to hold surface information
    pub surface_buffer: dare::render::resources::surface_buffer::RenderSurfaceBuffer<GPUAllocatorImpl>,
    /// staging buffers used
    pub staging_buffers: Vec<dagal::resource::Buffer<GPUAllocatorImpl>>,

    // cmd buffers
    pub command_pool: dagal::command::CommandPool,
    pub command_buffer: dagal::command::CommandBufferState,
}

impl Frame {
    pub fn new(
        surface_context: &SurfaceContext,
        present_queue: &dagal::device::Queue<tokio::sync::Mutex<vk::Queue>>,
        image_number: Option<usize>,
    ) -> Result<Self> {
        let mut allocator = surface_context.allocator.clone();
        let draw_image =
            dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
                device: surface_context.allocator.device(),
                allocator: &mut allocator,
                location: dagal::allocators::MemoryLocation::GpuOnly,
                image_ci: vk::ImageCreateInfo {
                    s_type: vk::StructureType::IMAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageCreateFlags::empty(),
                    image_type: vk::ImageType::TYPE_2D,
                    format: vk::Format::R16G16B16A16_SFLOAT,
                    extent: vk::Extent3D {
                        width: surface_context.image_extent.width,
                        height: surface_context.image_extent.height,
                        depth: 1,
                    },
                    mip_levels: 1,
                    array_layers: 1,
                    samples: vk::SampleCountFlags::TYPE_1,
                    tiling: vk::ImageTiling::OPTIMAL,
                    usage: vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::STORAGE,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    queue_family_index_count: 1,
                    p_queue_family_indices: &present_queue.get_family_index(),
                    initial_layout: vk::ImageLayout::UNDEFINED,
                    _marker: Default::default(),
                },
                name: Some(
                    image_number
                        .map_or(String::from("Swapchain image"), |image_number| {
                            format!("Swapchain image {:?}", image_number)
                        })
                        .as_str(),
                ),
            })?;
        let draw_image_view = dagal::resource::ImageView::new(
            dagal::resource::ImageViewCreateInfo::FromCreateInfo {
                device: surface_context.allocator.device(),
                create_info: vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: unsafe { *draw_image.as_raw() },
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: draw_image.format(),
                    components: Default::default(),
                    subresource_range:
                        dagal::resource::Image::<GPUAllocatorImpl>::image_subresource_range(
                            vk::ImageAspectFlags::COLOR,
                        ),
                    _marker: Default::default(),
                },
            },
        )?;
        let depth_image =
            dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
                device: surface_context.allocator.device(),
                allocator: &mut allocator,
                location: dagal::allocators::MemoryLocation::GpuOnly,
                image_ci: vk::ImageCreateInfo {
                    s_type: vk::StructureType::IMAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageCreateFlags::empty(),
                    image_type: vk::ImageType::TYPE_2D,
                    format: vk::Format::D32_SFLOAT,
                    extent: vk::Extent3D {
                        width: surface_context.image_extent.width,
                        height: surface_context.image_extent.height,
                        depth: 1,
                    },
                    mip_levels: 1,
                    array_layers: 1,
                    samples: vk::SampleCountFlags::TYPE_1,
                    tiling: vk::ImageTiling::OPTIMAL,
                    usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    queue_family_index_count: 1,
                    p_queue_family_indices: &present_queue.get_family_index(),
                    initial_layout: vk::ImageLayout::UNDEFINED,
                    _marker: Default::default(),
                },
                name: Some(
                    image_number
                        .map_or(String::from("Swapchain depth image"), |image_number| {
                            format!("Swapchain image {:?}", image_number)
                        })
                        .as_str(),
                ),
            })?;
        let depth_image_view = dagal::resource::ImageView::new(
            dagal::resource::ImageViewCreateInfo::FromCreateInfo {
                device: surface_context.allocator.device(),
                create_info: vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: unsafe { *depth_image.as_raw() },
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: depth_image.format(),
                    components: Default::default(),
                    subresource_range:
                        dagal::resource::Image::<GPUAllocatorImpl>::image_subresource_range(
                            vk::ImageAspectFlags::DEPTH,
                        ),
                    _marker: Default::default(),
                },
            },
        )?;
        let render_semaphore = dagal::sync::BinarySemaphore::new(
            surface_context.allocator.device(),
            vk::SemaphoreCreateFlags::empty(),
        )?;
        let swapchain_semaphore = dagal::sync::BinarySemaphore::new(
            surface_context.allocator.device(),
            vk::SemaphoreCreateFlags::empty(),
        )?;
        let render_fence = dagal::sync::Fence::new(
            surface_context.allocator.device(),
            vk::FenceCreateFlags::SIGNALED,
        )?;
        // make pools and buffers
        let command_pool = dagal::command::CommandPool::new(
            allocator.device(),
            present_queue,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )?;
        let command_buffer =
            dagal::command::CommandBufferState::from(command_pool.allocate(1)?.pop().unwrap());
        Ok(Frame {
            draw_image,
            draw_image_view,
            depth_image,
            depth_image_view,
            render_fence,
            render_semaphore,
            swapchain_semaphore,
            queue: present_queue.clone(),
            image_extent: surface_context.image_extent,

            resources: HashSet::default(),
            indirect_buffer: dare::render::util::GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: surface_context.allocator.device(),
                    name: Some(String::from(format!(
                        "Indirect buffer frame {}",
                        image_number.as_ref().unwrap_or(&0)
                    ))),
                    allocator: &mut allocator,
                    size: 128_000,
                    memory_type: MemoryLocation::GpuOnly,
                    usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::VERTEX_BUFFER,
                },
            )?,
            instanced_buffer: dare::render::util::GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: surface_context.allocator.device(),
                    name: Some(String::from(format!(
                        "Instanced buffer frame {}",
                        image_number.as_ref().unwrap_or(&0)
                    ))),
                    allocator: &mut allocator,
                    size: 128_000,
                    memory_type: MemoryLocation::GpuOnly,
                    usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::VERTEX_BUFFER,
                },
            )?,
            surface_buffer: dare::render::resources::RenderSurfaceBuffer::new(
                dare::render::util::GrowableBuffer::new(
                    dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                        device: surface_context.allocator.device(),
                        name: Some(String::from(format!(
                            "Render Surface Buffer {}",
                            image_number.as_ref().unwrap_or(&0)
                        ))),
                        allocator: &mut allocator,
                        size: 128_000,
                        memory_type: MemoryLocation::GpuOnly,
                        usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER
                            | vk::BufferUsageFlags::TRANSFER_DST
                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                            | vk::BufferUsageFlags::VERTEX_BUFFER,
                    },
                )?
            ),
            staging_buffers: Vec::new(),
            command_pool,
            command_buffer,
        })
    }

    /// Wait until the frame can be rendered into again
    pub async fn await_render(&self) -> Result<()> {
        //self.render_semaphore.clone().await;
        Ok(())
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        // Wait for render to finish
        if let Ok(status) = self.render_fence.get_fence_status() {
            if status {
                self.render_fence.wait(u64::MAX).unwrap()
            }
        }
    }
}
