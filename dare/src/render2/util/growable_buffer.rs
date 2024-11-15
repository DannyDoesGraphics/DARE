use dagal::allocators::Allocator;
use dagal::ash::vk;
use std::ops::Deref;

/// blocking changes i need to make:
/// TODO:
/// - port over [`vk::DeviceCreateInfo`] into our own custom struct to get rid of the lifetime
/// requirements
/// Describes a buffer which can grow dynamically, but shrinks rarely
pub struct GrowableBuffer<A: Allocator> {
    handle: Option<dagal::resource::Buffer<A>>,
}
impl<A: Allocator> Deref for GrowableBuffer<A> {
    type Target = dagal::resource::Buffer<A>;

    fn deref(&self) -> &Self::Target {
        self.handle.as_ref().unwrap()
    }
}

impl<A: Allocator> GrowableBuffer<A> {
    pub fn new(handle: dagal::resource::Buffer<A>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    /// Grows the current buffer by [`dl`]
    pub fn grow(&mut self, dl: vk::DeviceSize) -> anyhow::Result<()> {
        todo!()
    }

    /// Shrinks the current by [`dl`] and effectively cuts off the last [`dl`] bytes
    pub fn shrink(&mut self, dl: vk::DeviceSize) -> anyhow::Result<()> {
        todo!()
    }
}
