use std::{marker::PhantomData, ptr};

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
        self.command_infos.push(vk::CommandBufferSubmitInfo {
            s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
            p_next: ptr::null(),
            command_buffer: unsafe {
                *command.as_raw()
            },
            device_mask: 0,
            _marker: PhantomData::default(),
        });
        self.idx += 1;
        SubmitBatchCommandHandle(self.idx)
    }

    pub fn then(
        &mut self,
        previous: SubmitBatchCommandHandle,
        stage: vk::PipelineStageFlags2,
        command: &'a crate::command::CommandBufferExecutable,
    ) -> crate::Result<SubmitBatchCommandHandle> {
        self.semaphores.push(crate::sync::BinarySemaphore::new(
            self.device.clone(),
            vk::SemaphoreCreateFlags::empty(),
        )?);

        self.semaphore_infos.push(vk::SemaphoreSubmitInfo {
            s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
            p_next: ptr::null(),
            device_index: 0,
            stage_mask: stage,
            value: 0,
            semaphore: unsafe { *self.semaphores.last().unwrap().as_raw() },
            _marker: PhantomData::default(),
        });
        self.idx += 1;
        Ok(SubmitBatchCommandHandle(self.idx))
    }
}
