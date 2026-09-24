use dagal::util::tests::TestHarness;

/// Headless Vulkan instance
pub struct TestContext {
    allocator: dagal::allocators::GPUAllocatorImpl,
    headless: TestHarness,
}

impl TestContext {
    pub fn new() -> anyhow::Result<Self> {
        let headless = TestHarness::headless().build()?;
        Ok(Self {
            allocator: headless.allocator(),
            headless,
        })
    }

    /// Get the logical device
    pub fn device(&self) -> dagal::device::LogicalDevice {
        self.headless.device()
    }

    /// Get the queue
    pub fn queue_info(&self) -> dagal::device::QueueInfo {
        self.headless.queue_info()
    }

    /// Acquire an owned handle to the `index`th active queue.
    ///
    /// `vkGetDeviceQueue2` hands back the same underlying queue for a given
    /// family/index pair, so this does not allocate a new queue. Two queues were
    /// requested at init, so index 0 is free for a caller that needs one of its own
    /// while [`Self::immediate_submit`] keeps using the context's.
    pub fn queue(&self, index: usize) -> dagal::device::Queue {
        self.headless.queue(index)
    }

    /// Get the allocator
    pub fn allocator(&self) -> dagal::allocators::GPUAllocatorImpl {
        self.allocator.clone()
    }

    /// Perform immediate submission of GPU commands and wait on their completion
    pub fn immediate_submit<
        F: FnOnce(&TestHarness, &dagal::command::CommandBufferRecording) -> R,
        R,
    >(
        &self,
        f: F,
    ) -> dagal::Result<R> {
        self.headless.immediate_submit(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dagal::ash::vk;
    use dagal::command::command_buffer::CmdBuffer;
    use dagal::resource::traits::Resource;
    use dagal::traits::AsRaw;
    use serial_test::serial;

    /// Literally test if the context can be created
    #[test]
    fn test_new() {
        let context = TestContext::new().unwrap();
        drop(context);
    }

    #[test]
    #[serial]
    fn test_immediate_submit() {
        let mut context = TestContext::new().unwrap();

        // Create a small buffer for testing
        let buffer =
            dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: context.device(),
                name: Some("TestBuffer".to_string()),
                allocator: &mut context.allocator,
                size: 64,
                memory_type: dagal::allocators::MemoryLocation::GpuToCpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST,
            })
            .unwrap();

        // Fill buffer with 0xDEADBEEF pattern using GPU command
        let pattern: u32 = 0xDEADBEEF;
        context
            .immediate_submit(|_context, recording| unsafe {
                recording.get_device().get_handle().cmd_fill_buffer(
                    *recording.as_raw(),
                    *buffer.as_raw(),
                    0,
                    64,
                    pattern,
                );
            })
            .unwrap();

        // Read back and verify the data
        let data = buffer.read::<u32>(0, 16).unwrap();
        for (i, val) in data.iter().enumerate() {
            assert_eq!(
                *val, pattern,
                "Buffer word {} should be 0x{:08X}, got 0x{:08X}",
                i, pattern, val
            );
        }
    }
}
