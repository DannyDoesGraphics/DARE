use dagal::ash::vk;

/// A present context for managing presentation operations
#[derive(Debug)]
pub struct PresentContext {
    pub frame_index: u64,
    pub frames: Vec<crate::frame::Frame>,
    pub present_semaphores: Vec<dagal::sync::BinarySemaphore>,
}

impl PresentContext {
    pub fn new(core_context: &super::CoreContext, frames_in_flight: usize) -> dagal::Result<Self> {
        let mut frames: Vec<crate::frame::Frame> = Vec::with_capacity(frames_in_flight);

        for _ in 0..frames_in_flight {
            let command_pool = dagal::command::CommandPool::new(
                dagal::command::CommandPoolCreateInfo::WithQueue {
                    device: core_context.device.clone(),
                    flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    queue: &core_context.queues.present,
                },
            )?;
            let command_buffer = command_pool.allocate(1).unwrap().pop().unwrap();

            frames.push(crate::frame::Frame {
                render_fence: dagal::sync::Fence::new(
                    core_context.device.clone(),
                    vk::FenceCreateFlags::SIGNALED,
                )?,
                swapchain_semaphore: dagal::sync::BinarySemaphore::new(
                    core_context.device.clone(),
                    vk::SemaphoreCreateFlags::empty(),
                )?,
                command_pool,
                command_buffer,
            });
        }

        Ok(Self {
            frame_index: 0,
            frames,
            present_semaphores: Vec::new(),
        })
    }

    pub fn rebuild_present_semaphores(
        &mut self,
        device: &dagal::device::LogicalDevice,
        image_count: usize,
    ) -> dagal::Result<()> {
        match self.present_semaphores.len().cmp(&image_count) {
            std::cmp::Ordering::Equal => Ok(()),
            std::cmp::Ordering::Less => {
                while self.present_semaphores.len() < image_count {
                    self.present_semaphores
                        .push(dagal::sync::BinarySemaphore::new(
                            device.clone(),
                            vk::SemaphoreCreateFlags::empty(),
                        )?);
                }
                Ok(())
            }
            std::cmp::Ordering::Greater => {
                self.wait_gpu_idle(device)?;
                self.present_semaphores.truncate(image_count);
                Ok(())
            }
        }
    }

    fn wait_gpu_idle(&self, device: &dagal::device::LogicalDevice) -> dagal::Result<()> {
        for frame in &self.frames {
            frame.render_fence.wait(u64::MAX)?;
        }
        unsafe {
            device
                .get_handle()
                .device_wait_idle()
                .map_err(dagal::DagalError::VkError)?;
        }
        Ok(())
    }
}
