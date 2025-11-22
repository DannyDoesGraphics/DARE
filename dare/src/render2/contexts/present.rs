use dagal::ash::vk;

/// A present context for managing presentation operations
#[derive(Debug, bevy_ecs::resource::Resource)]
pub struct PresentContext {
    pub command_pool: dagal::command::CommandPool,
    pub command_buffer: Option<dagal::command::CommandBuffer>,
    pub image_available_semaphore: dagal::sync::BinarySemaphore,
    pub render_finished_semaphore: dagal::sync::BinarySemaphore,
    pub in_flight_fence: dagal::sync::Fence,
}

impl PresentContext {
    pub fn new(core_context: &super::CoreContext) -> dagal::Result<Self> {
        let command_pool =
            dagal::command::CommandPool::new(dagal::command::CommandPoolCreateInfo::WithQueue {
                device: core_context.device.clone(),
                flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                queue: &core_context.present_queue,
            })?;
        let image_available_semaphore = dagal::sync::BinarySemaphore::new(
            core_context.device.clone(),
            vk::SemaphoreCreateFlags::empty(),
        )?;
        let render_finished_semaphore = dagal::sync::BinarySemaphore::new(
            core_context.device.clone(),
            vk::SemaphoreCreateFlags::empty(),
        )?;
        let mut allocated_buffers = command_pool.allocate(1)?;
        let command_buffer = allocated_buffers
            .pop()
            .expect("failed to allocate present command buffer");
        let in_flight_fence =
            dagal::sync::Fence::new(core_context.device.clone(), vk::FenceCreateFlags::SIGNALED)?;

        Ok(Self {
            command_pool,
            command_buffer: Some(command_buffer),
            image_available_semaphore,
            render_finished_semaphore,
            in_flight_fence,
        })
    }
}
