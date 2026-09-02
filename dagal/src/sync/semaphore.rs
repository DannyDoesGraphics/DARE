use ash::vk;

use crate::traits::{AsRaw, Destructible};

#[derive(Debug, PartialEq, Eq)]
pub struct Semaphore(super::BinarySemaphore);

impl Semaphore {
    pub fn new(
        flags: vk::SemaphoreCreateFlags,
        device: crate::device::LogicalDevice,
        initial_value: u64,
    ) -> Result<Self, crate::DagalError> {
        let mut type_ci = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(initial_value);
        let handle = unsafe {
            device.get_handle().create_semaphore(
                &vk::SemaphoreCreateInfo::default()
                    .flags(flags)
                    .push_next(&mut type_ci),
                None,
            )?
        };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating VkSemaphore {:p}", handle);

        Ok(Self(super::BinarySemaphore::from_handle(device, handle)))
    }

    /// Signal a semaphore to a given value
    pub fn signal(&self, value: u64) -> Result<(), crate::DagalError> {
        unsafe {
            self.0.device.get_handle().signal_semaphore(
                &vk::SemaphoreSignalInfo::default()
                    .semaphore(self.0.handle)
                    .value(value),
            )?;
        }
        Ok(())
    }

    /// Get semaphore current value
    pub fn current_value(&self) -> Result<u64, crate::DagalError> {
        Ok(unsafe {
            self.0
                .device
                .get_handle()
                .get_semaphore_counter_value(self.0.handle)
        }?)
    }
}

impl Destructible for Semaphore {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        log::trace!("Destroying VkSemaphore {:p}", self.0.handle);
        unsafe {
            self.0
                .device
                .get_handle()
                .destroy_semaphore(self.0.handle, None);
        }
    }
}

impl AsRaw for Semaphore {
    type RawType = vk::Semaphore;

    unsafe fn raw(self) -> Self::RawType {
        self.0.handle
    }

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.0.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.0.handle
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        // Empty drop to prevent double destruction
    }
}
