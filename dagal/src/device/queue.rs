use std::hash::{Hash, Hasher};
use std::sync::Arc;
#[cfg(not(feature = "tokio"))]
use std::sync::{Mutex, MutexGuard};
#[cfg(feature = "tokio")]
use tokio::sync::{Mutex, MutexGuard};

#[allow(unused_imports)]
use crate::DagalError;
#[allow(unused_imports)]
use anyhow::Result;
use ash::vk;

use crate::prelude as dagal;

/// Quick easy abstraction over queues

/// Represents a [`vk::Queue`] and it's indices
#[derive(Debug)]
pub struct Queue<M: dagal::concurrency::Lockable<Target=vk::Queue> = dagal::DEFAULT_LOCKABLE<vk::Queue>> {
    /// Handle to [`vk::Queue`]
    handle: Arc<M>,

    /// Index to the family queue
    family_index: u32,

    /// Queue's index in the family
    index: u32,

    /// Whether the queue is dedicated
    dedicated: bool,

    /// Flags of the queue
    queue_flags: vk::QueueFlags,
}
impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Clone for Queue<M> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            family_index: self.family_index,
            index: self.index,
            dedicated: self.dedicated,
            queue_flags: self.queue_flags,
        }
    }
}
impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> PartialEq for Queue<M> {
    fn eq(&self, other: &Self) -> bool {
        self.family_index == other.family_index && self.index == other.index
    }
}
impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Eq for Queue<M> {}
impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Hash for Queue<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.family_index.hash(state);
        self.index.hash(state);
    }
}
unsafe impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Send for Queue<M> {}
impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Queue<M> {
    pub fn get_index(&self) -> u32 {
        self.index
    }

    pub fn get_family_index(&self) -> u32 {
        self.family_index
    }

    pub fn get_dedicated(&self) -> bool {
        self.dedicated
    }

    pub fn get_queue_flags(&self) -> vk::QueueFlags {
        self.queue_flags
    }
}

impl<M: dagal::concurrency::Lockable<Target=vk::Queue>> Queue<M> {
    /// It is undefined behavior to pass in a [`vk:Queue`] from an already existing [`Queue`]
    pub unsafe fn new(
        handle: vk::Queue,
        family_index: u32,
        index: u32,
        dedicated: bool,
        queue_flags: vk::QueueFlags,
    ) -> Self {
        Self {
            handle: Arc::new(M::new(handle)),
            family_index,
            index,
            dedicated,
            queue_flags,
        }
    }

    pub fn get_handle(&self) -> &M {
        &self.handle
    }
}

impl<M: dagal::concurrency::SyncLockable<Target=vk::Queue>> Queue<M> {

    pub fn acquire_queue_lock<'a>(&'a self) -> Result<M::Lock<'a>> {
        self.handle.lock()
    }

    pub fn try_queue_lock<'a>(&'a self) -> Result<M::Lock<'a>> {
        self.handle.try_lock()
    }
}

impl<M: dagal::concurrency::AsyncLockable<Target=vk::Queue>> Queue<M> {
    pub async fn acquire_queue_async<'a>(&'a self) -> Result<M::Lock<'a>> {
        Ok(self.handle.lock().await?)
    }

    pub fn acquire_queue_blocking<'a>(&'a self) -> M::Lock<'a> {
        self.handle.blocking_lock().unwrap()
    }

    pub fn try_queue_lock_async<'a>(&'a self) -> Result<M::Lock<'a>> {
        Ok(self.handle.try_lock()?)
    }
}