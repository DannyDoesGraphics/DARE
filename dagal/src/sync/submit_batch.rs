use crate::traits::*;
use ash::vk;

/// An opaque handle back to a command buffer stored in [`SubmitBatch`]
pub struct SubmitBatchCommandHandle(u32);

/// Allows for the synchronization + submission of multiple command buffers within a single queue.
#[derive(Debug)]
pub struct SubmitBatch<'a> {
    device: crate::device::LogicalDevice,
    idx: u32,
    command_infos: Vec<vk::CommandBufferSubmitInfo<'a>>,
    semaphores: Vec<crate::sync::BinarySemaphore>,
    semaphore_infos: Vec<vk::SemaphoreSubmitInfo<'a>>,
}

impl<'a> SubmitBatch<'a> {
    /// Add a command buffer into the SubmitBatch and receive back an associated opaque handle
    pub fn submit(
        &mut self,
        command: &'a crate::command::CommandBufferExecutable,
    ) -> SubmitBatchCommandHandle {
        self.command_infos.push(
            vk::CommandBufferSubmitInfo::default().command_buffer(unsafe { *command.as_raw() }),
        );
        self.idx += 1;
        SubmitBatchCommandHandle(self.idx)
    }

    pub fn then(
        &mut self,
        _previous: SubmitBatchCommandHandle,
        stage: vk::PipelineStageFlags2,
        _command: &'a crate::command::CommandBufferExecutable,
    ) -> crate::Result<SubmitBatchCommandHandle> {
        self.semaphores.push(crate::sync::BinarySemaphore::new(
            self.device.clone(),
            vk::SemaphoreCreateFlags::empty(),
        )?);

        self.semaphore_infos.push(
            vk::SemaphoreSubmitInfo::default()
                .stage_mask(stage)
                .semaphore(unsafe { *self.semaphores.last().unwrap().as_raw() }),
        );
        self.idx += 1;
        Ok(SubmitBatchCommandHandle(self.idx))
    }
}
