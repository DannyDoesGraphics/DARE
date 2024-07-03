use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Result;
use ash::vk;

use crate::DagalError;

/// Quick easy abstraction over queues

/// Represents a [`vk::Queue`] and it's indices
#[derive(Clone, Debug)]
pub struct Queue {
    /// Handle to [`vk::Queue`]
    handle: Arc<Mutex<vk::Queue>>,

    /// Index to the family queue
    family_index: u32,

    /// Queue's index in the family
    index: u32,

    /// Whether the queue is dedicated
    dedicated: bool,

    /// Flags of the queue
    queue_flags: vk::QueueFlags,
}

impl PartialEq for Queue {
    fn eq(&self, other: &Self) -> bool {
        self.family_index == other.family_index && self.index == other.index
    }
}
impl Eq for Queue {}

impl Queue {
    /// It is undefined behavior to pass in a [`vk:Queue`] from an already existing [`Queue`]
    pub unsafe fn new(
        handle: vk::Queue,
        family_index: u32,
        index: u32,
        dedicated: bool,
        queue_flags: vk::QueueFlags,
    ) -> Self {
        Self {
            handle: Arc::new(Mutex::new(handle)),
            family_index,
            index,
            dedicated,
            queue_flags,
        }
    }

    /// Get the underlying reference to [`VkQueue`](vk::Queue)
    pub fn get_handle(&self) -> &Arc<Mutex<vk::Queue>> {
        &self.handle
    }

    pub fn acquire_queue_lock(&self) -> Result<MutexGuard<vk::Queue>> {
        Ok(self.handle.lock().map_err(|_| DagalError::PoisonError)?)
    }

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
