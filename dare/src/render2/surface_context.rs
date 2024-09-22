use std::sync::Arc;
use anyhow::Result;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::traits::AsRaw;
use dagal::winit;
use bevy_ecs::prelude as becs;
use tokio::sync::RwLock;

/// Relating to anything that relies on window resizing
#[derive(Debug, becs::Resource)]
pub struct SurfaceContext {
    pub surface: dagal::wsi::SurfaceQueried,
    pub swapchain: dagal::wsi::Swapchain,
    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub image_extent: vk::Extent2D,
    pub frames: Arc<[super::frame::Frame]>,


    pub swapchain_images: Arc<[dagal::resource::Image<GPUAllocatorImpl>]>,
    pub swapchain_image_view: Arc<[dagal::resource::ImageView]>,
    pub swapchain_image_index: RwLock<u32>,
    
    pub frames_in_flight: usize,
}

pub struct SurfaceContextCreateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub physical_device: &'a dagal::device::PhysicalDevice,
    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub window: Arc<winit::window::Window>,

    pub frames_in_flight: Option<usize>
}

/// Information to create a window context
pub(super) struct InnerSurfaceContextCreateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub physical_device: &'a dagal::device::PhysicalDevice,
    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub present_queue: dagal::device::Queue,
    pub window: &'a winit::window::Window,

    // Frames in flight
    pub frames_in_flight: Option<usize>,
}

impl SurfaceContext {
    pub fn new(window_context_ci: InnerSurfaceContextCreateInfo) -> Result<Self> {
        // expect present queue with graphics bit
        if window_context_ci.present_queue.get_queue_flags() & vk::QueueFlags::GRAPHICS != vk::QueueFlags::GRAPHICS {
            return Err(anyhow::anyhow!("Expected a queue flag with GRAPHICS, got queue bit flag: {:?}", window_context_ci.present_queue.get_queue_flags()))
        }
        // make instances
        let surface = dagal::wsi::Surface::new(
            window_context_ci.instance.get_entry(),
            window_context_ci.instance.get_instance(),
            window_context_ci.window
        )?;
        let surface = surface.query_details(unsafe {
            *window_context_ci.physical_device.as_raw()
        })?;
        let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface);
        // clamp window size into surface limits
        let image_extent = swapchain.clamp_extent(&vk::Extent2D {
            width: window_context_ci.window.inner_size().width,
            height: window_context_ci.window.inner_size().height,
        });
        let frames_in_flight = window_context_ci.frames_in_flight.map(|fif| fif.clamp(
            surface.get_capabilities().min_image_count as usize,
            surface.get_capabilities().max_image_count as usize
        ) as u32);
        // rebuild swapchain
        let swapchain = swapchain
            .push_queue(&window_context_ci.present_queue)
            .min_image_count(frames_in_flight)
            .request_present_mode(vk::PresentModeKHR::MAILBOX)
            .request_present_mode(vk::PresentModeKHR::FIFO)
            .request_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .request_image_format(vk::Format::B8G8R8A8_UNORM)
            .set_extent(image_extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .build(window_context_ci.instance.get_instance(), window_context_ci.allocator.get_device().clone())?;
        let swapchain_images: Arc<[dagal::resource::Image<GPUAllocatorImpl>]> = Arc::from(swapchain.get_images::<GPUAllocatorImpl>()?.into_boxed_slice());
        let swapchain_image_view: Arc<[dagal::resource::ImageView]> = Arc::from(swapchain.get_image_views(&
            swapchain_images.iter().map(|image| unsafe {*image.as_raw()}).collect::<Vec<vk::Image>>()
        )?.into_boxed_slice());
        let frames_in_flight = frames_in_flight.unwrap_or(surface.get_capabilities().min_image_count) as usize;

        Ok(SurfaceContext {
            surface,
            swapchain,
            allocator: window_context_ci.allocator,
            image_extent,
            frames: Arc::from(Vec::new().into_boxed_slice()),
            swapchain_images,
            swapchain_image_view,
            swapchain_image_index: RwLock::new(0),
            
            frames_in_flight,
        })
    }
}


