use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::traits::{AsRaw, Destructible};
use dagal::winit;
use std::mem::{swap, ManuallyDrop};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Relating to anything that relies on window resizing
#[derive(Debug)]
pub struct SurfaceContext {
    pub swapchain_images: Box<[std::sync::Mutex<dagal::resource::Image<GPUAllocatorImpl>>]>,
    pub swapchain_image_view: Box<[dagal::resource::ImageView]>,
    pub swapchain_image_index: RwLock<u32>,

    pub image_extent: vk::Extent2D,
    pub frames: Box<[Mutex<super::frame::Frame>]>,

    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub swapchain: dagal::wsi::Swapchain,
    pub surface: dagal::wsi::SurfaceQueried,

    pub frames_in_flight: usize,
}

pub struct SurfaceContextUpdateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub physical_device: &'a dagal::device::PhysicalDevice,
    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub window: &'a winit::window::Window,

    pub frames_in_flight: Option<usize>,
}

/// Information to create a window context
pub(super) struct InnerSurfaceContextCreateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub surface: Option<dagal::wsi::Surface>,
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
        if window_context_ci.present_queue.get_queue_flags() & vk::QueueFlags::TRANSFER
            != vk::QueueFlags::TRANSFER
        {
            return Err(anyhow::anyhow!(
                "Expected a queue flag with TRANSFER, got queue bit flag: {:?}",
                window_context_ci.present_queue.get_queue_flags()
            ));
        }
        // make instances
        let surface = window_context_ci.surface.unwrap_or(dagal::wsi::Surface::new(
            window_context_ci.instance.get_entry(),
            window_context_ci.instance.get_instance(),
            window_context_ci.window,
        )?);
        let surface =
            surface.query_details(unsafe { *window_context_ci.physical_device.as_raw() })?;
        let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface);
        // clamp window size into surface limits
        let image_extent = swapchain.clamp_extent(&vk::Extent2D {
            width: window_context_ci.window.inner_size().width,
            height: window_context_ci.window.inner_size().height,
        });
        let frames_in_flight = window_context_ci.frames_in_flight.map(|fif| {
            fif.clamp(
                surface.get_capabilities().min_image_count as usize,
                surface.get_capabilities().max_image_count as usize,
            ) as u32
        });
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
            .build(
                window_context_ci.instance.get_instance(),
                window_context_ci.allocator.get_device().clone(),
            )?;
        let swapchain_images: Vec<dagal::resource::Image<GPUAllocatorImpl>> =
            swapchain.get_images::<GPUAllocatorImpl>()?;
        let swapchain_image_view: Box<[dagal::resource::ImageView]> = swapchain
            .get_image_views(
                &swapchain_images
                    .iter()
                    .map(|image| unsafe { *image.as_raw() })
                    .collect::<Vec<vk::Image>>(),
            )?
            .into_boxed_slice();
        let swapchain_images: Box<[std::sync::Mutex<dagal::resource::Image<GPUAllocatorImpl>>]> =
            swapchain_images
                .into_iter()
                .map(|image: dagal::resource::Image<GPUAllocatorImpl>| std::sync::Mutex::new(image))
                .collect::<Vec<std::sync::Mutex<dagal::resource::Image<GPUAllocatorImpl>>>>()
                .into_boxed_slice();
        let frames_in_flight =
            frames_in_flight.unwrap_or(surface.get_capabilities().min_image_count) as usize;
        Ok(SurfaceContext {
            surface,
            swapchain,
            allocator: window_context_ci.allocator,
            image_extent,
            frames: Vec::new().into_boxed_slice(),
            swapchain_images,
            swapchain_image_view,
            swapchain_image_index: RwLock::new(0),

            frames_in_flight,
        })
    }

    /// Create frames for the window context
    pub fn create_frames(&mut self, present_queue: &dagal::device::Queue) -> Result<()> {
        let mut frames = Vec::with_capacity(self.frames_in_flight);
        for frame_number in 0..self.frames_in_flight {
            frames.push(Mutex::new(super::frame::Frame::new(
                self,
                present_queue,
                Some(frame_number),
            )?));
        }
        self.frames = frames.into_boxed_slice();
        Ok(())
    }
}

impl Drop for SurfaceContext {
    fn drop(&mut self) {
        use std::ptr;
        let mut vk_fences: Vec<vk::Fence> = Vec::new();
        while vk_fences.len() != self.frames.len() {
            vk_fences.clear();
            for frame in self.frames.iter() {
                tokio::task::block_in_place(|| {
                    let rt_handle = tokio::runtime::Handle::current();
                    rt_handle.block_on(async {
                        let locked_frame = frame.lock().await;
                        if locked_frame.render_fence.get_fence_status().unwrap_or(true) {
                            vk_fences.push(unsafe { *locked_frame.render_fence.as_raw() });
                        }
                    });
                });
            }
        }
        if !vk_fences.is_empty() {
            unsafe {
                self.allocator
                    .device()
                    .get_handle()
                    .wait_for_fences(&vk_fences, true, u64::MAX)
                    .unwrap()
            }
        }
    }
}
