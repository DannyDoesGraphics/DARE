use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::ptr;
use tracing::trace;

#[derive(Debug, Clone)]
pub struct CommandPool {
    handle: vk::CommandPool,
    device: crate::device::LogicalDevice,
}

impl CommandPool {
    pub fn new(
        device: crate::device::LogicalDevice,
        queue: &crate::device::Queue,
        flags: vk::CommandPoolCreateFlags,
    ) -> Result<Self> {
        let command_pool_ci = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags,
            queue_family_index: queue.get_family_index(),
            _marker: Default::default(),
        };
        let handle = unsafe {
            device
                .get_handle()
                .create_command_pool(&command_pool_ci, None)?
        };

        #[cfg(feature = "log-lifetimes")]
        trace!("Created VkCommandPool {:p}", handle);

        Ok(Self { handle, device })
    }

    pub fn handle(&self) -> vk::CommandPool {
        self.handle
    }

    pub fn get_handle(&self) -> &vk::CommandPool {
        &self.handle
    }

    pub fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    /// Allocate command buffers from a command pool
    pub fn allocate(&self, count: u32) -> Result<Vec<crate::command::CommandBuffer>> {
        Ok(unsafe {
            self.device
                .get_handle()
                .allocate_command_buffers(&vk::CommandBufferAllocateInfo {
                    s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
                    p_next: ptr::null(),
                    command_pool: self.handle,
                    level: vk::CommandBufferLevel::PRIMARY,
                    command_buffer_count: count,
                    _marker: Default::default(),
                })
        }?
        .into_iter()
        .map(|buffer| crate::command::CommandBuffer::new(buffer, self.device.clone()))
        .collect::<Vec<crate::command::CommandBuffer>>())
    }
}

impl Destructible for CommandPool {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkCommandPool {:p}", self.handle);

        unsafe {
            self.device
                .get_handle()
                .destroy_command_pool(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for CommandPool {
    fn drop(&mut self) {
        self.destroy();
    }
}
