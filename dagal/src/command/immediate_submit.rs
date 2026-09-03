use ash::vk;

use crate::traits::AsRaw;

#[derive(Debug)]
pub struct ImmediateSubmit {
    device: crate::device::LogicalDevice,
    queue: crate::device::Queue,
    pool: crate::command::CommandPool,
    command_buffer: Option<crate::command::CommandBuffer>,
    fence: crate::sync::Fence,
}

impl ImmediateSubmit {
    pub fn new(
        device: crate::device::LogicalDevice,
        queue: crate::device::Queue,
    ) -> crate::Result<Self> {
        let pool =
            crate::command::CommandPool::new(crate::command::CommandPoolCreateInfo::WithQueue {
                device: device.clone(),
                queue: &queue,
                flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            })?;
        let command_buffer = pool.allocate(1)?.pop();
        let fence = crate::sync::Fence::new(device.clone(), vk::FenceCreateFlags::empty())?;
        Ok(Self {
            device,
            queue,
            pool,
            command_buffer,
            fence,
        })
    }

    pub fn queue(&self) -> &crate::device::Queue {
        &self.queue
    }

    pub fn device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    /// Blocks all submit calls
    pub fn submit<F, R>(&mut self, recorder: F) -> crate::Result<R>
    where
        F: FnOnce(&crate::command::CommandBufferRecording) -> R,
    {
        let command_buffer = match self.command_buffer.take() {
            Some(command_buffer) => {
                command_buffer.reset(vk::CommandBufferResetFlags::empty())?;
                command_buffer
            }
            None => self
                .pool
                .allocate(1)?
                .pop()
                .ok_or(crate::DagalError::VkError(vk::Result::ERROR_UNKNOWN))?,
        };

        let recording = command_buffer
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .map_err(|invalid| invalid.error())?;
        let recorded = recorder(&recording);
        let executable = recording.end().map_err(|error| {
            error
                .downcast::<vk::Result>()
                .map(crate::DagalError::VkError)
                .unwrap_or(crate::DagalError::VkError(vk::Result::ERROR_UNKNOWN))
        })?;

        let command_infos = [executable.submit_info()];
        let submits = [vk::SubmitInfo2::default().command_buffer_infos(&command_infos)];
        self.fence.reset()?;
        let command_buffer = executable
            .submit(unsafe { *self.queue.as_raw() }, &submits, unsafe {
                *self.fence.as_raw()
            })
            .map_err(|invalid| invalid.error())?;
        self.fence.wait(u64::MAX)?;
        self.command_buffer = Some(command_buffer);

        Ok(recorded)
    }
}

#[cfg(test)]
mod tests {
    use ash::vk;

    use crate::command::command_buffer::CmdBuffer;
    use crate::resource::traits::Resource;
    use crate::traits::AsRaw;
    use crate::util::tests::TestHarness;

    fn readback(
        harness: &TestHarness,
    ) -> crate::resource::Buffer<crate::allocators::GPUAllocatorImpl> {
        crate::resource::Buffer::new(crate::resource::BufferCreateInfo::NewEmptyBuffer {
            device: harness.device(),
            name: None,
            allocator: &harness.allocator(),
            size: 64,
            memory_type: crate::allocators::MemoryLocation::GpuToCpu,
            usage_flags: vk::BufferUsageFlags::TRANSFER_DST,
        })
        .unwrap()
    }

    fn fill(
        harness: &TestHarness,
        buffer: &crate::resource::Buffer<crate::allocators::GPUAllocatorImpl>,
        pattern: u32,
    ) {
        harness
            .immediate_submit(|_, recording| unsafe {
                recording.get_device().get_handle().cmd_fill_buffer(
                    *recording.as_raw(),
                    *buffer.as_raw(),
                    0,
                    64,
                    pattern,
                );
            })
            .unwrap();
    }

    #[test]
    fn submit_waits_for_the_gpu_before_returning() {
        let harness = TestHarness::headless().build().unwrap();
        let buffer = readback(&harness);

        fill(&harness, &buffer, 0xABAD_1DEA);

        assert_eq!(buffer.read::<u32>(0, 16).unwrap(), &[0xABAD_1DEA; 16]);
    }

    #[test]
    fn harness_methods_are_callable_from_inside_a_submit() {
        let harness = TestHarness::headless().build().unwrap();
        let family = harness
            .immediate_submit(|harness, _| harness.queue_info().family_index)
            .unwrap();
        assert_eq!(family, harness.queue_info().family_index);
    }

    #[test]
    fn consecutive_submits_reuse_one_command_buffer() {
        let harness = TestHarness::headless().build().unwrap();
        let buffer = readback(&harness);

        fill(&harness, &buffer, 0x1111_1111);
        fill(&harness, &buffer, 0x2222_2222);
        fill(&harness, &buffer, 0x3333_3333);

        assert_eq!(buffer.read::<u32>(0, 16).unwrap(), &[0x3333_3333; 16]);
    }
}
