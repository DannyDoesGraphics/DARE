use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;

#[derive(Debug, Clone)]
pub struct ImageView {
    handle: vk::ImageView,
    device: crate::device::LogicalDevice,
}

impl ImageView {
    pub fn new(
        image_view_ci: &vk::ImageViewCreateInfo,
        device: crate::device::LogicalDevice,
    ) -> Result<Self> {
        let handle = unsafe { device.get_handle().create_image_view(image_view_ci, None)? };
        Ok(Self { handle, device })
    }

    pub fn from_vk(image_view: vk::ImageView, device: crate::device::LogicalDevice) -> Self {
        Self {
            handle: image_view,
            device,
        }
    }

    pub fn get_handle(&self) -> &vk::ImageView {
        &self.handle
    }

    pub fn handle(&self) -> vk::ImageView {
        self.handle
    }
}

impl Destructible for ImageView {
    fn destroy(&mut self) {
        unsafe {
            self.device
                .get_handle()
                .destroy_image_view(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for ImageView {
    fn drop(&mut self) {
        self.destroy();
    }
}
