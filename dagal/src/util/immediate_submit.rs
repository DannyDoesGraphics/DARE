use anyhow::Result;
use ash::vk;

use crate::command::command_buffer::CmdBuffer;
use crate::traits::Destructible;

/// Adds a basic struct which can immediately submit all commands
#[derive(Debug)]
pub struct ImmediateSubmit {
    fence: crate::sync::Fence,
    command_buffer: crate::command::CommandBuffer,
    command_pool: crate::command::CommandPool,
    device: crate::device::LogicalDevice,
    queue: crate::device::Queue,
}

#[derive(Debug)]
pub struct ImmediateSubmitContext<'a> {
    pub device: &'a crate::device::LogicalDevice,
    pub cmd: &'a crate::command::CommandBufferRecording,
    pub queue: &'a crate::device::Queue,
}

impl Destructible for ImmediateSubmit {
    fn destroy(&mut self) {
        self.fence.destroy();
        self.command_pool.destroy();
    }
}

impl ImmediateSubmit {
    pub fn new(device: crate::device::LogicalDevice, queue: crate::device::Queue) -> Result<Self> {
        let fence = crate::sync::Fence::new(device.clone(), vk::FenceCreateFlags::SIGNALED)?;
        let command_pool = crate::command::CommandPool::new(
            device.clone(),
            &queue,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )?;
        let command_buffer = command_pool.allocate(1)?.pop().unwrap();
        Ok(Self {
            fence,
            command_pool,
            command_buffer,
            device,
            queue,
        })
    }

    /// Immediately submit a function which fills out a command buffer
    pub fn submit<T: FnOnce(ImmediateSubmitContext)>(&self, function: T) {
        unsafe {
            self.device
                .get_handle()
                .reset_fences(&[self.fence.handle()])
                .unwrap();
            self.command_buffer
                .reset(vk::CommandBufferResetFlags::empty())
                .unwrap();
        }
        let cmd = self
            .command_buffer
            .clone()
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();
        let context = ImmediateSubmitContext {
            device: &self.device,
            cmd: &cmd,
            queue: &self.queue,
        };
        function(context);
        let cmd = cmd.end().unwrap();
        let raw_cmd = cmd.handle();
        cmd.submit(
            self.queue.handle(),
            &[crate::command::CommandBufferExecutable::submit_info_sync(
                &[crate::command::CommandBufferExecutable::submit_info(
                    raw_cmd,
                )],
                &[],
                &[],
            )],
            self.fence.handle(),
        )
        .unwrap();
        unsafe {
            self.fence.wait(9999999999).unwrap_unchecked();
        }
    }

    /// Get a reference to the underlying device
    pub fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    pub fn get_queue(&self) -> &crate::device::Queue {
        &self.queue
    }
}
