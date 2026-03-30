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
        let type_ci = vk::SemaphoreTypeCreateInfo {
            s_type: vk::StructureType::SEMAPHORE_TYPE_CREATE_INFO,
            p_next: std::ptr::null(),
            semaphore_type: vk::SemaphoreType::TIMELINE,
            initial_value,
            _marker: std::marker::PhantomData,
        };
        let handle = unsafe {
            device.get_handle().create_semaphore(
                &vk::SemaphoreCreateInfo {
                    s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
                    p_next: &type_ci as *const _ as *const std::ffi::c_void,
                    flags,
                    _marker: Default::default(),
                },
                None,
            )?
        };

        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Creating VkSemaphore {:p}", handle);

        Ok(Self(super::BinarySemaphore {
            device,
            handle
        }))
    }

    /// Signal a semaphore to a given value
    pub fn signal(&self, value: u64) -> Result<(), crate::DagalError> {
        unsafe {
            self.0.device
                .get_handle()
                .signal_semaphore(&vk::SemaphoreSignalInfo {
                    s_type: vk::StructureType::SEMAPHORE_SIGNAL_INFO,
                    p_next: std::ptr::null(),
                    semaphore: self.0.handle,
                    value,
                    _marker: std::marker::PhantomData,
                })?;
        }
        Ok(())
    }

    /// Get semaphore current value
    pub fn current_value(&self) -> Result<u64, crate::DagalError> {
        Ok(unsafe {
            self.0.device
                .get_handle()
                .get_semaphore_counter_value(self.0.handle)
        }?)
    }
    
    /// AsRef the timeline semaphore as a binary one
    pub(crate) fn as_binary_semaphore(&self) -> &super::BinarySemaphore {
        &self.0
    }
    
    /// AsMut the timeline semaphore as a binary one
    pub(crate) fn mut_binary_semaphore(&mut self) -> &mut super::BinarySemaphore {
        &mut self.0
    }
}

impl Destructible for Semaphore {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkSemaphore {:p}", self.handle);
        unsafe {
            self.0.device
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

#[cfg(feature = "raii")]
impl Drop for Semaphore {
    fn drop(&mut self) {
        self.destroy();
    }
}
