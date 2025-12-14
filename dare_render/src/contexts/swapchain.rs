use std::ptr;

use dare_window::WindowHandles;
use dagal::{allocators::Allocator, ash::vk, resource::traits::Resource, traits::AsRaw};

/// A rendering context for window-based rendering
#[derive(Debug, bevy_ecs::resource::Resource)]
pub struct SwapchainContext<A: Allocator> {
    frames: Vec<crate::frame::SwapchainFrame<A>>,
    pub swapchain: dagal::wsi::Swapchain,
    pub extent: vk::Extent2D,
    surface: dagal::wsi::SurfaceQueried,
}

impl<A: Allocator> SwapchainContext<A> {
    /// Select a swapchain extent based on surface capabilities and requested extent
    fn select_extent(
        capabilities: &vk::SurfaceCapabilitiesKHR,
        requested: vk::Extent2D,
    ) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: requested.width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: requested.height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        }
    }

    /// Select the number of images for the swapchain based on surface capabilities and preferred count
    fn select_image_count(capabilities: &vk::SurfaceCapabilitiesKHR, preferred: u32) -> u32 {
        let min = capabilities.min_image_count.max(1);
        if capabilities.max_image_count == 0 {
            preferred.max(min)
        } else {
            preferred.max(min).min(capabilities.max_image_count)
        }
    }

    /// Create a new SwapchainContext
    pub fn new(
        surface: dagal::wsi::SurfaceQueried,
        extent: vk::Extent2D,
        core_context: &super::CoreContext,
    ) -> dagal::Result<Self> {
        let capabilities = surface.get_capabilities();
        let image_extent = Self::select_extent(&capabilities, extent);
        let image_count = Self::select_image_count(&capabilities, u32::MAX);
        let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface)
            .min_image_count(Some(image_count))
            .request_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .request_present_mode(vk::PresentModeKHR::MAILBOX)
            .request_image_format(vk::Format::B8G8R8A8_SRGB)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .set_extent(image_extent)
            .build(&core_context.instance, core_context.device.clone())?;

        let mut slf = Self {
            swapchain,
            surface,
            extent: image_extent,
            frames: Vec::new(),
        };
        slf.rebuild_frames()?;

        Ok(slf)
    }

    pub fn image_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame_mut(
        &mut self,
        index: usize,
    ) -> Option<&mut crate::frame::SwapchainFrame<A>> {
        self.frames.get_mut(index)
    }

    pub fn swapchain_handle(&self) -> &vk::SwapchainKHR {
        self.swapchain.get_handle()
    }

    /// Queue a present operation for the swapchain
    pub fn queue_present(
        &self,
        queue: vk::Queue,
        present_info: &vk::PresentInfoKHR,
    ) -> Result<(), vk::Result> {
        unsafe { self.swapchain.get_ext().queue_present(queue, present_info) }.map(|_| ())
    }

    /// Resize the swapchain to the new extent
    pub fn resize(
        &mut self,
        extent: vk::Extent2D,
        present_context: &super::PresentContext,
        core_context: &super::CoreContext,
    ) -> dagal::Result<()> {
        if extent.width == 0 || extent.height == 0 {
            return Ok(());
        }
        unsafe {
            let fences: Vec<vk::Fence> = present_context
                .frames
                .iter()
                .map(|f| *f.render_fence.as_raw())
                .collect::<Vec<vk::Fence>>();
            core_context
                .device
                .get_handle()
                .wait_for_fences(&fences, true, u64::MAX)
                .unwrap();
        }
        self.surface
            .refresh(*core_context.physical_device.get_handle())?;
        let capabilities = self.surface.get_capabilities();
        let image_extent = Self::select_extent(&capabilities, extent);
        if image_extent.width == 0 || image_extent.height == 0 {
            return Ok(());
        }
        let image_count = Self::select_image_count(&capabilities, 3);
        let swapchain_ci = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.surface.handle(),
            min_image_count: image_count,
            image_format: vk::Format::B8G8R8A8_SRGB,
            image_color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            image_extent,
            image_array_layers: 1,
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            pre_transform: capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: vk::PresentModeKHR::MAILBOX,
            clipped: vk::TRUE,
            old_swapchain: *self.swapchain.get_handle(),
            _marker: std::marker::PhantomData,
        };
        self.swapchain = dagal::wsi::Swapchain::new(
            core_context.instance.get_instance(),
            core_context.device.clone(),
            &swapchain_ci,
        )?;
        self.extent = image_extent;
        self.rebuild_frames()?;

        Ok(())
    }

    /// Recreate the swapchain context with new window handles and extent
    pub fn recreate(
        &mut self,
        extent: vk::Extent2D,
        handles: WindowHandles,
        present_context: &super::PresentContext,
        core_context: &super::CoreContext,
    ) -> dagal::Result<()> {
        let surface: dagal::wsi::SurfaceQueried = dagal::wsi::Surface::new_with_handles(
            core_context.instance.get_entry(),
            core_context.instance.get_instance(),
            *handles.raw_display_handle,
            *handles.raw_window_handle,
        )?
        .query_details(*core_context.physical_device.get_handle())?;
        self.surface = surface;
        self.resize(extent, present_context, core_context)?;

        Ok(())
    }

    /// Clear out old swapchain images and create new ones
    fn rebuild_frames(&mut self) -> dagal::Result<()> {
        let images: Vec<dagal::resource::Image<A>> = self.swapchain.get_images::<A>()?;
        let mut frames: Vec<crate::frame::SwapchainFrame<A>> =
            Vec::with_capacity(images.len());
        for (index, image) in images.into_iter().enumerate() {
            let image_view = dagal::resource::ImageView::new(
                dagal::resource::ImageViewCreateInfo::FromCreateInfo {
                    device: image.get_device().clone(),
                    create_info: vk::ImageViewCreateInfo {
                        s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::ImageViewCreateFlags::empty(),
                        image: unsafe { *image.as_raw() },
                        view_type: vk::ImageViewType::TYPE_2D,
                        format: vk::Format::B8G8R8A8_SRGB,
                        components: vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        },
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        _marker: std::marker::PhantomData,
                    },
                    name: Some(format!("present_image_{index}")),
                },
            )?;
            frames.push(crate::frame::SwapchainFrame { image, image_view });
        }
        self.frames = frames;
        Ok(())
    }
}
