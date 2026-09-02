use ash::vk;

use crate::traits::{AsRaw, Destructible};

#[derive(Debug, PartialEq, Eq)]
pub struct BinarySemaphore {
    pub(super) handle: vk::Semaphore,
    pub(super) device: crate::device::LogicalDevice,
}

impl BinarySemaphore {
    pub fn new(
        device: crate::device::LogicalDevice,
        flags: vk::SemaphoreCreateFlags,
    ) -> crate::Result<Self> {
        let handle = unsafe {
            device
                .get_handle()
                .create_semaphore(&vk::SemaphoreCreateInfo::default().flags(flags), None)?
        };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating binary VkSemaphore {:p}", handle);

        Ok(Self { handle, device })
    }

    pub(super) fn from_handle(device: crate::device::LogicalDevice, handle: vk::Semaphore) -> Self {
        Self { handle, device }
    }

    pub fn get_handle(&self) -> &vk::Semaphore {
        &self.handle
    }

    pub fn handle(&self) -> vk::Semaphore {
        self.handle
    }

    /// Quickly get submission info for a single semaphore
    pub fn submit_info(
        &self,
        stage_mask: vk::PipelineStageFlags2,
    ) -> vk::SemaphoreSubmitInfo<'static> {
        vk::SemaphoreSubmitInfo::default()
            .semaphore(self.handle)
            .value(0)
            .stage_mask(stage_mask)
            .device_index(0)
    }
}

impl Destructible for BinarySemaphore {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        log::trace!("Destroying binary VkSemaphore {:p}", self.handle);

        unsafe {
            self.device
                .get_handle()
                .destroy_semaphore(self.handle, None);
        }
    }
}

impl AsRaw for BinarySemaphore {
    type RawType = vk::Semaphore;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}

impl Drop for BinarySemaphore {
    fn drop(&mut self) {
        self.destroy();
    }
}
