use std::ptr;
use crate::traits::Destructible;
use ash::vk;
use anyhow::Result;

use tracing::trace;

#[derive(Debug, Clone)]
pub struct DescriptorSetLayout {
    handle: vk::DescriptorSetLayout,
    device: crate::device::LogicalDevice,
}

impl DescriptorSetLayout {
    /// Get a copy of the underlying Vulkan object
    pub fn handle(&self) -> vk::DescriptorSetLayout {
        self.handle
    }

    pub fn from_raw(handle: vk::DescriptorSetLayout, device: crate::device::LogicalDevice) -> Self {
        Self { handle, device }
    }
}

impl Destructible for DescriptorSetLayout {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkDescriptorLayout {:p}", self.handle);
        unsafe {
            self.device
                .get_handle()
                .destroy_descriptor_set_layout(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        self.destroy();
    }
}
