use anyhow::Result;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::traits::AsRaw;

/// Relating to anything that relies on window resizing
#[derive(Debug)]
pub struct SurfaceContext {
    pub swapchain_images: Box<[dagal::resource::Image<GPUAllocatorImpl>]>,
    pub swapchain_image_view: Box<[dagal::resource::ImageView]>,
    pub swapchain_image_index: u32,

    pub image_extent: vk::Extent2D,
    pub frames: Box<[super::super::frame::Frame]>,

    pub allocator: GPUAllocatorImpl,
    pub swapchain: dagal::wsi::Swapchain,
    pub surface: dagal::wsi::SurfaceQueried,

    pub frames_in_flight: usize,
}

pub struct SurfaceContextUpdateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub physical_device: &'a dagal::device::PhysicalDevice,
    pub allocator: GPUAllocatorImpl,
    pub raw_handles: crate::window::WindowHandles,
    pub dimensions: Option<(u32, u32)>,

    pub frames_in_flight: Option<usize>,
}

/// Information to create a window context
pub struct InnerSurfaceContextCreateInfo<'a> {
    pub instance: &'a dagal::core::Instance,
    pub surface: Option<dagal::wsi::Surface>,
    pub physical_device: &'a dagal::device::PhysicalDevice,
    pub allocator: GPUAllocatorImpl,
    pub present_queue: dagal::device::Queue,
    pub raw_handles: crate::window::WindowHandles,
    pub extent: (u32, u32),

    // Frames in flight
    pub frames_in_flight: Option<usize>,
}

impl SurfaceContext {
    pub fn new(window_context_ci: InnerSurfaceContextCreateInfo<'_>) -> Result<Self> {
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
        let surface = window_context_ci
            .surface
            .unwrap_or(dagal::wsi::Surface::new_with_handles(
                window_context_ci.instance.get_entry(),
                window_context_ci.instance.get_instance(),
                *window_context_ci.raw_handles.raw_display_handle,
                *window_context_ci.raw_handles.raw_window_handle,
            )?);
        let surface =
            surface.query_details(unsafe { *window_context_ci.physical_device.as_raw() })?;
        let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface);
        // clamp window size into surface limits
        let image_extent = swapchain.clamp_extent(&vk::Extent2D {
            width: window_context_ci.extent.0,
            height: window_context_ci.extent.1,
        });
        // Get surface capabilities once and reuse
        let surface_capabilities = surface.get_capabilities();
        let frames_in_flight = window_context_ci.frames_in_flight.map(|fif| {
            fif.clamp(
                surface_capabilities.min_image_count as usize,
                surface_capabilities.max_image_count as usize,
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
        let swapchain_images: Box<[dagal::resource::Image<GPUAllocatorImpl>]> =
            swapchain_images.into_boxed_slice();
        let frames_in_flight =
            frames_in_flight.unwrap_or(surface_capabilities.min_image_count) as usize;
        Ok(SurfaceContext {
            surface,
            swapchain,
            allocator: window_context_ci.allocator,
            image_extent,
            frames: Vec::new().into_boxed_slice(),
            swapchain_images,
            swapchain_image_view,
            swapchain_image_index: 0,

            frames_in_flight,
        })
    }

    /// Create frames for the window context
    pub fn create_frames(&mut self, present_queue: &dagal::device::Queue) -> Result<()> {
        let mut frames = Vec::with_capacity(self.frames_in_flight);
        for frame_number in 0..self.frames_in_flight {
            frames.push(super::super::frame::Frame::new(
                self,
                present_queue,
                Some(frame_number),
            )?);
        }
        self.frames = frames.into_boxed_slice();
        Ok(())
    }

    /// Get surface capabilities
    pub fn get_surface_capabilities(&self) -> vk::SurfaceCapabilitiesKHR {
        self.surface.get_capabilities()
    }

    /// Get surface formats
    pub fn get_surface_formats(&self) -> &[vk::SurfaceFormatKHR] {
        self.surface.get_formats()
    }

    /// Get surface present modes
    pub fn get_surface_present_modes(&self) -> &[vk::PresentModeKHR] {
        self.surface.get_present_modes()
    }

    /// Get the surface handle
    pub fn get_surface_handle(&self) -> vk::SurfaceKHR {
        self.surface.handle()
    }

    /// Get direct access to the surface queried (for cases where the convenience methods aren't enough)
    pub fn get_surface(&self) -> &dagal::wsi::SurfaceQueried {
        &self.surface
    }
}

impl Drop for SurfaceContext {
    fn drop(&mut self) {
        let mut vk_fences = Vec::new();

        // Collect all valid fences
        for frame in self.frames.iter() {
            if frame.render_fence.get_fence_status().unwrap_or(true) {
                vk_fences.push(unsafe { *frame.render_fence.as_raw() });
            }
        }

        // Wait for all fences if any were collected
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
