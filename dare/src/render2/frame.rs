use std::cell::{Cell, RefCell};
use std::ptr;
use std::sync::Arc;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use anyhow::Result;
use dagal::resource::traits::Resource;
use bevy_ecs::prelude as becs;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};
use crate::render2::surface_context::SurfaceContext;

/// Contains all information necessary to render current frame
#[derive(Debug)]
pub struct Frame {
    // Image that is being drawn to is here
    pub draw_image: dagal::resource::Image<GPUAllocatorImpl>,
    pub render_fence: dagal::sync::Fence,
    pub render_semaphore: dagal::sync::BinarySemaphore,
    pub swapchain_semaphore: dagal::sync::BinarySemaphore,
    pub queue: dagal::device::Queue,
    pub image_extent: vk::Extent2D,

    // cmd buffers
    pub command_pool: dagal::command::CommandPool,
    pub command_buffer: dagal::command::CommandBufferState,
}

impl Frame {
    pub async fn new(surface_context: &SurfaceContext, present_queue: &dagal::device::Queue, image_number: Option<usize>) -> Result<Self> {

        let mut allocator = surface_context.allocator.clone();
        let draw_image = dagal::resource::Image::new(
            dagal::resource::ImageCreateInfo::NewAllocated {
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
                name: Some(image_number.map_or(String::from("Swapchain image"), |image_number| format!("Swapchain image {:?}", image_number)).as_str()),
            }
        )?;
        let render_semaphore = dagal::sync::BinarySemaphore::new(
            surface_context.allocator.device(),
            vk::SemaphoreCreateFlags::empty()
        )?;
        let swapchain_semaphore = dagal::sync::BinarySemaphore::new(
            surface_context.allocator.device(),
            vk::SemaphoreCreateFlags::empty()
        )?;
        let render_fence = dagal::sync::Fence::new(
            surface_context.allocator.device(),
            vk::FenceCreateFlags::SIGNALED
        )?;
        // make pools and buffers
        let command_pool = dagal::command::CommandPool::new(
            allocator.device(),
            present_queue,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
        )?;
        let command_buffer = dagal::command::CommandBufferState::from(command_pool.allocate(1)?.pop().unwrap());
        println!("FRAME CREATED");
        Ok(Frame {
            draw_image,
            render_fence,
            render_semaphore,
            swapchain_semaphore,
            queue: present_queue.clone(),
            image_extent: surface_context.image_extent,

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