use std::ptr;
use crate::concurrency::DEFAULT_LOCKABLE;
use std::sync::Arc;
use ash::vk;
use crate::bootstrap::app_info::{Expected, QueueRequest};

#[derive(Debug, Clone)]
pub struct ExecutionManager<M: crate::concurrency::lockable::TryLockable<Target = vk::Queue> = DEFAULT_LOCKABLE<vk::Queue>> {
    device: crate::device::LogicalDevice,
    queues: Arc<[crate::device::Queue<M>]>,
}

impl<M: crate::concurrency::lockable::TryLockable<Target = vk::Queue>> ExecutionManager<M> {
    pub fn from_queues(device: crate::device::LogicalDevice, queues: Vec<crate::device::Queue<M>>) -> Self {
        Self {
            device,
            queues: queues.into(),
        }
    }

    /// Acquire a queue s.t. it can perform presents
    pub fn acquire_present_queue(&self) -> Option<M::Lock<'_>> {
        self.queues.iter().find_map(|queue| {
            if queue.can_present() {
                queue.try_queue_lock().ok()
            } else {
                None
            }
        })
    }

    /// Works like [`Self::from_queues`], but makes an execution manager over the entire device
    pub fn from_device(device: crate::device::LogicalDevice, physical_device: &crate::device::PhysicalDevice) -> Self {
        Self {
            device: device.clone(),
            queues: physical_device.get_active_queues().iter().map(|q| {
                unsafe {
                    device.get_queue(&vk::DeviceQueueInfo2 {
                        s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
                        p_next: ptr::null(),
                        flags: Default::default(),
                        queue_family_index: q.family_index,
                        queue_index: q.index,
                        _marker: Default::default(),
                    },
                                     q.queue_flags,
                                     q.strict,
                                     q.can_present
                    )
                }
            }).collect::<Vec<crate::device::Queue<M>>>().into(),
        }
    }
}