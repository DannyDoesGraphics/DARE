use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

#[derive(Debug)]
struct ImageHandleInner<A: Allocator = GPUAllocatorImpl> {
    handle: Handle<resource::Image<A>>,
    gpu_rt: GPUResourceTable<A>,
}

impl<A: Allocator> PartialEq for ImageHandleInner<A> {
    fn eq(&self, other: &Self) -> bool {
        self.handle.eq(&other.handle)
    }
}

impl<A: Allocator> Drop for ImageHandleInner<A> {
    fn drop(&mut self) {
        self.gpu_rt.free_image(self.handle.clone()).unwrap()
    }
}

/// An arc reference to a handle which is dropped at lifetime end
#[derive(Debug, Clone, PartialEq)]
pub struct ImageHandle<A: Allocator = GPUAllocatorImpl> {
    inner: Arc<ImageHandleInner<A>>,
}

impl<A: Allocator> ImageHandle<A> {
    pub fn new(handle: Handle<resource::Image<A>>, gpu_rt: GPUResourceTable<A>) -> Self {
        Self {
            inner: Arc::new(ImageHandleInner { handle, gpu_rt }),
        }
    }

    /// Get the underlying handle
    pub fn get_handle(&self) -> &Handle<resource::Image<A>> {
        &self.inner.handle
    }
}
