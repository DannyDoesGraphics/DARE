use ash::vk;

use crate::device::{LogicalDevice, PhysicalDevice, Queue};
use crate::DagalError;

/// Named device queues assigned at init.
#[derive(Debug)]
pub struct QueueRegistry {
    pub present: Queue,
    pub transfer: Option<Queue>,
    pub spare: Vec<Queue>,
}

impl QueueRegistry {
    pub fn from_device(
        device: &LogicalDevice,
        physical_device: &PhysicalDevice,
    ) -> crate::Result<Self> {
        let queues = physical_device
            .get_active_queues()
            .iter()
            .map(|queue_info| {
                let queue_ci: vk::DeviceQueueInfo2<'_> = (*queue_info).into();
                unsafe {
                    device.get_queue(
                        &queue_ci,
                        queue_info.queue_flags,
                        queue_info.strict,
                        queue_info.can_present,
                    )
                }
            })
            .collect();
        Self::from_queues(queues)
    }

    pub fn from_queues(mut queues: Vec<Queue>) -> crate::Result<Self> {
        if queues.is_empty() {
            return Err(DagalError::ImpossibleQueue.into());
        }
        let present_idx = queues.iter().position(|q| q.can_present()).unwrap_or(0);
        let present = queues.remove(present_idx);

        let transfer_idx = queues
            .iter()
            .position(|q| q.get_queue_flags().contains(vk::QueueFlags::TRANSFER));
        let transfer = transfer_idx.map(|i| queues.remove(i));

        Ok(Self {
            present,
            transfer,
            spare: queues,
        })
    }

    pub fn take_transfer(&mut self) -> crate::Result<Queue> {
        self.transfer
            .take()
            .ok_or(DagalError::ImpossibleQueue.into())
    }
}
