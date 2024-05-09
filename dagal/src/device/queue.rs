use ash::vk;

/// Quick easy abstraction over queues

/// Represents a [`vk::Queue`] and it's indices
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct Queue {
    /// Handle to [`vk::Queue`]
    handle: vk::Queue,

    /// Index to the family queue
    family_index: u32,

    /// Queue's index in the family
    index: u32,
}

impl Queue {
    pub fn new(handle: vk::Queue, family_index: u32, index: u32) -> Self {
        Self {
            handle,
            family_index,
            index,
        }
    }

    /// Get the underlying reference to [`VkQueue`](vk::Queue)
    pub fn get_handle(&self) -> &vk::Queue {
        &self.handle
    }

    /// Get the underlying copy of [`VkQueue`](vk::Queue)
    pub fn handle(&self) -> vk::Queue {
        self.handle
    }

    pub fn get_index(&self) -> u32 {
        self.index
    }

    pub fn get_family_index(&self) -> u32 {
        self.family_index
    }
}
