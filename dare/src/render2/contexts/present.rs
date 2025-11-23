use dagal::ash::vk;

/// A present context for managing presentation operations
#[derive(Debug, bevy_ecs::resource::Resource)]
pub struct PresentContext {
    pub frame_index: u64,
    pub frames: Vec<crate::render2::frame::Frame>,
}

impl PresentContext {
    pub fn new(core_context: &super::CoreContext, frames_in_flight: usize) -> dagal::Result<Self> {
        let mut frames: Vec<crate::render2::frame::Frame> = Vec::with_capacity(frames_in_flight);

        for _ in 0..frames_in_flight {
            let command_pool = dagal::command::CommandPool::new(
                dagal::command::CommandPoolCreateInfo::WithQueue {
                    device: core_context.device.clone(),
                    flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                    queue: &core_context.present_queue,
                },
            )?;
            let command_buffer = command_pool.allocate(1).unwrap().pop().unwrap();

            frames.push(crate::render2::frame::Frame {
                render_fence: dagal::sync::Fence::new(
                    core_context.device.clone(),
                    vk::FenceCreateFlags::SIGNALED,
                )?,
                render_semaphore: dagal::sync::BinarySemaphore::new(
                    core_context.device.clone(),
                    vk::SemaphoreCreateFlags::empty(),
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
        })
    }
}

impl Drop for PresentContext {
    fn drop(&mut self) {
        for frame in &self.frames {
            frame.render_fence.wait(u64::MAX).unwrap();
        }
    }
}
