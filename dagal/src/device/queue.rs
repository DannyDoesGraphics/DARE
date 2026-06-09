use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use crate::traits::AsRaw;
use crate::DagalError;
use anyhow::Result;
use ash::vk::{self};

/// Information about queues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueueInfo {
    /// Index to the family queue
    pub family_index: u32,
    /// Queue's index in the family
    pub index: u32,
    /// Whether the queue is dedicated
    pub strict: bool,
    /// Flags of the queue
    pub queue_flags: vk::QueueFlags,
    /// Can the queue present to the device's surface
    pub can_present: bool,
}

impl From<QueueInfo> for vk::DeviceQueueInfo2<'_> {
    fn from(val: QueueInfo) -> Self {
        vk::DeviceQueueInfo2 {
            s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
            p_next: std::ptr::null(),
            flags: vk::DeviceQueueCreateFlags::empty(),
            queue_family_index: val.family_index,
            queue_index: val.index,
            _marker: Default::default(),
        }
    }
}

/// Represents a [`vk::Queue`] and it's indices
///
/// # Hashing
/// When hashing, the hasher will only hash [`Self::index`] and [`Self::family_index`]
#[derive(Debug)]
pub struct Queue {
    /// Handle to [`vk::Queue`]
    handle: vk::Queue,
    device: crate::device::LogicalDevice,
    queue_info: QueueInfo,
    _not_sync: PhantomData<*const ()>,
}

unsafe impl Send for Queue {}

impl PartialEq for Queue {
    fn eq(&self, other: &Self) -> bool {
        self.queue_info.family_index == other.queue_info.family_index
            && self.queue_info.index == other.queue_info.index
    }
}

impl Eq for Queue {}

impl Hash for Queue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.queue_info.family_index.hash(state);
        self.queue_info.index.hash(state);
    }
}

impl Queue {
    pub fn get_index(&self) -> u32 {
        self.queue_info.index
    }

    pub fn get_family_index(&self) -> u32 {
        self.queue_info.family_index
    }

    pub fn get_dedicated(&self) -> bool {
        self.queue_info.strict
    }

    pub fn get_queue_flags(&self) -> vk::QueueFlags {
        self.queue_info.queue_flags
    }

    pub fn can_present(&self) -> bool {
        self.queue_info.can_present
    }

    pub fn get_info(&self) -> QueueInfo {
        self.queue_info
    }

    pub fn device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    /// It is undefined behavior to pass in a [`vk:Queue`] from an already existing [`Queue`]
    pub unsafe fn new(
        device: crate::device::LogicalDevice,
        handle: vk::Queue,
        queue_info: QueueInfo,
    ) -> Self {
        Self {
            handle,
            device,
            queue_info,
            _not_sync: PhantomData,
        }
    }

    pub fn submit2(
        &self,
        submit_infos: &[vk::SubmitInfo2<'_>],
        fence: vk::Fence,
    ) -> Result<(), DagalError> {
        unsafe {
            self.device
                .get_handle()
                .queue_submit2(self.handle, submit_infos, fence)
        }
        .map_err(DagalError::VkError)
    }

    pub fn submit2_and_wait_fence(
        &self,
        submit_infos: &[vk::SubmitInfo2<'_>],
        fence: &crate::sync::Fence,
    ) -> Result<(), DagalError> {
        self.submit2(submit_infos, unsafe { *fence.as_raw() })?;
        fence.wait(u64::MAX)?;
        Ok(())
    }
}

impl AsRaw for Queue {
    type RawType = vk::Queue;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}
