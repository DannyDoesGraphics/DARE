use std::ptr;

use ash::vk;

use crate::traits::Destructible;

#[derive(Debug)]
pub struct CommandPool {
    handle: vk::CommandPool,
    device: crate::device::LogicalDevice,
}

pub enum CommandPoolCreateInfo<'a> {
    WithQueue {
        device: crate::device::LogicalDevice,
        flags: vk::CommandPoolCreateFlags,
        queue: &'a crate::device::Queue,
    },
    WithQueueFamily {
        device: crate::device::LogicalDevice,
        flags: vk::CommandPoolCreateFlags,
        queue_family_index: u32,
    },
}
impl CommandPoolCreateInfo<'_> {
    pub fn flags(&self) -> vk::CommandPoolCreateFlags {
        match self {
            CommandPoolCreateInfo::WithQueue { flags, .. } => *flags,
            CommandPoolCreateInfo::WithQueueFamily { flags, .. } => *flags,
        }
    }

    pub fn device(&self) -> &crate::device::LogicalDevice {
        match self {
            CommandPoolCreateInfo::WithQueue { device, .. } => device,
            CommandPoolCreateInfo::WithQueueFamily { device, .. } => device,
        }
    }
}

impl CommandPool {
    pub fn new(ci: CommandPoolCreateInfo) -> crate::Result<Self> {
        let command_pool_ci = vk::CommandPoolCreateInfo {
            s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: ci.flags(),
            queue_family_index: match &ci {
                CommandPoolCreateInfo::WithQueue { queue, .. } => queue.get_family_index(),
                CommandPoolCreateInfo::WithQueueFamily {
                    queue_family_index, ..
                } => *queue_family_index,
            },
            _marker: Default::default(),
        };
        let handle = unsafe {
            ci.device()
                .get_handle()
                .create_command_pool(&command_pool_ci, None)?
        };

        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Created VkCommandPool {:p}", handle);

        Ok(Self {
            handle,
            device: ci.device().clone(),
        })
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

    /// Allocate primary command buffers from this command pool
    pub fn allocate(&self, count: u32) -> crate::Result<Vec<crate::command::CommandBuffer>> {
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
        tracing::trace!("Destroying VkCommandPool {:p}", self.handle);

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
