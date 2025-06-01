use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::Arc;
#[cfg(not(feature = "tokio"))]
use std::sync::{Mutex, MutexGuard};

use crate::prelude as dagal;
#[allow(unused_imports)]
use crate::DagalError;
#[allow(unused_imports)]
use anyhow::Result;
use ash::vk;

/// Information about queues
#[derive(Debug, Clone, PartialEq, Eq)]
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
            p_next: ptr::null(),
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
pub struct Queue<
    M: dagal::concurrency::Lockable<Target = vk::Queue> = dagal::DEFAULT_LOCKABLE<vk::Queue>,
> {
    /// Handle to [`vk::Queue`]
    handle: Arc<M>,
    queue_info: QueueInfo,
}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Clone for Queue<M> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            queue_info: self.queue_info.clone(),
        }
    }
}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> PartialEq for Queue<M> {
    fn eq(&self, other: &Self) -> bool {
        self.queue_info.family_index == other.queue_info.family_index
            && self.queue_info.index == other.queue_info.index
    }
}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Eq for Queue<M> {}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Hash for Queue<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.queue_info.family_index.hash(state);
        self.queue_info.index.hash(state);
    }
}
unsafe impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Send for Queue<M> {}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Queue<M> {
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
}

impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> Queue<M> {
    /// It is undefined behavior to pass in a [`vk:Queue`] from an already existing [`Queue`]
    pub unsafe fn new(handle: vk::Queue, queue_info: QueueInfo) -> Self {
        Self {
            handle: Arc::new(M::new(handle)),
            queue_info,
        }
    }

    pub fn get_handle(&self) -> &M {
        &self.handle
    }
}

impl<M: dagal::concurrency::SyncLockable<Target = vk::Queue>> Queue<M> {
    pub fn acquire_queue_lock(&self) -> Result<M::Lock<'_>> {
        self.handle.lock()
    }
}

impl<M: dagal::concurrency::TryLockable<Target = vk::Queue>> Queue<M> {
    pub fn try_queue_lock(&self) -> Result<M::Lock<'_>> {
        self.handle.try_lock()
    }
}

impl<M: dagal::concurrency::AsyncLockable<Target = vk::Queue>> Queue<M> {
    pub async fn acquire_queue_async<'a>(&'a self) -> Result<M::Lock<'a>> {
        self.handle.lock().await
    }

    pub fn acquire_queue_blocking(&self) -> M::Lock<'_> {
        self.handle.blocking_lock().unwrap()
    }
}
