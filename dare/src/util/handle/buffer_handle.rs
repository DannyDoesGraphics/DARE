use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

#[derive(Debug)]
struct BufferHandleInner<A: Allocator = GPUAllocatorImpl> {
    handle: Handle<resource::Buffer<A>>,
    gpu_rt: GPUResourceTable<A>,
}

impl<A: Allocator> PartialEq for BufferHandleInner<A> {
    fn eq(&self, other: &Self) -> bool {
        self.handle.eq(&other.handle)
    }
}

impl<A: Allocator> Drop for BufferHandleInner<A> {
    fn drop(&mut self) {
        self.gpu_rt.free_buffer(self.handle.clone()).unwrap()
    }
}

/// An arc reference to a handle which is dropped at lifetime end
#[derive(Debug, Clone, PartialEq)]
pub struct BufferHandle<A: Allocator = GPUAllocatorImpl> {
    inner: Arc<BufferHandleInner<A>>,
}

impl<A: Allocator> BufferHandle<A> {
    pub fn new(handle: Handle<resource::Buffer<A>>, gpu_rt: GPUResourceTable<A>) -> Self {
        Self {
            inner: Arc::new(BufferHandleInner { handle, gpu_rt }),
        }
    }

    /// Get the underlying handle
    pub fn get_handle(&self) -> Handle<resource::Buffer<A>> {
        self.inner.handle.clone()
    }
}
