use crate::traits::Destructible;
use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;

use crate::resource::traits::Resource;
use std::ptr;
use tracing::trace;

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Swapchain {
    handle: vk::SwapchainKHR,
    #[derivative(Debug = "ignore")]
    ext: ash::khr::swapchain::Device,
    device: crate::device::LogicalDevice,

    format: vk::Format,
    extent: vk::Extent2D,

    usage_flags: vk::ImageUsageFlags,
}

pub struct SwapchainImageInfo {
    format: vk::Format,
    extent: vk::Extent2D,
}

impl Swapchain {
    /// Construct a basic swapchain. For an easier build of a swapchain, see
    /// [`bootstrap::SwapchainBuilder`](crate::bootstrap::SwapchainBuilder).
    pub fn new(
        instance: &ash::Instance,
        device: crate::device::LogicalDevice,
        swapchain_ci: &vk::SwapchainCreateInfoKHR,
    ) -> Result<Self> {
        let ext = ash::khr::swapchain::Device::new(instance, device.get_handle());
        let handle = unsafe { ext.create_swapchain(swapchain_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkSwapchainKHR {:p}", handle);

        Ok(Self {
            handle,
            ext: ash::khr::swapchain::Device::new(instance, device.get_handle()),
            device,
            format: swapchain_ci.image_format,
            extent: swapchain_ci.image_extent,
            usage_flags: swapchain_ci.image_usage,
        })
    }

    /// Get the underlying [`VkSwapchainKHR`](vk::SwapchainKHR)
    pub fn get_handle(&self) -> &vk::SwapchainKHR {
        &self.handle
    }

    /// Get the underlying device extension
    pub fn get_ext(&self) -> &ash::khr::swapchain::Device {
        &self.ext
    }

    pub fn get_images(&self) -> Result<Vec<crate::resource::Image>> {
        Ok(unsafe { self.ext.get_swapchain_images(self.handle)? }
            .into_iter()
            .map(|image| {
                crate::resource::Image::new(crate::resource::ImageCreateInfo::FromVkNotManaged {
                    device: self.device.clone(),
                    image,
                    format: self.format,
                    extent: vk::Extent3D {
                        width: self.extent.width,
                        height: self.extent.height,
                        depth: 1,
                    },
                })
                .unwrap()
            })
            .collect::<Vec<crate::resource::Image>>())
    }

    pub fn get_image_views(&self, images: &[vk::Image]) -> Result<Vec<crate::resource::ImageView>> {
        images
            .iter()
            .map(|image| {
                crate::resource::ImageView::new(
                    crate::resource::ImageViewCreateInfo::FromCreateInfo {
                        create_info: vk::ImageViewCreateInfo {
                            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::ImageViewCreateFlags::empty(),
                            image: *image,
                            view_type: vk::ImageViewType::TYPE_2D,
                            format: self.format,
                            components: Default::default(),
                            subresource_range: vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            },
                            _marker: Default::default(),
                        },
                        device: self.device.clone(),
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn next_image_index(
        &self,
        timeout: u64,
        semaphore: Option<crate::sync::BinarySemaphore>,
        fence: Option<crate::sync::Fence>,
    ) -> Result<u32> {
        unsafe {
            Ok(self
                .ext
                .acquire_next_image(
                    self.handle,
                    timeout,
                    semaphore.map_or(vk::Semaphore::null(), |semaphore| semaphore.handle()),
                    fence.map_or(vk::Fence::null(), |fence| fence.handle()),
                )
                .map(|res| res.0)?)
        }
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }
}

impl Destructible for Swapchain {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkSwapchainKHR {:p}", self.handle);

        unsafe {
            self.ext.destroy_swapchain(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for Swapchain {
    fn drop(&mut self) {
        self.destroy();
    }
}
