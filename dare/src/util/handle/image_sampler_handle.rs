use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

#[derive(Debug)]
struct SamplerHandleInner<A: Allocator = GPUAllocatorImpl> {
    handle: Handle<resource::Sampler>,
    gpu_rt: GPUResourceTable<A>,
}

impl<A: Allocator> PartialEq for SamplerHandleInner<A> {
    fn eq(&self, other: &Self) -> bool {
        self.handle.eq(&other.handle)
    }
}

impl<A: Allocator> Drop for SamplerHandleInner<A> {
    fn drop(&mut self) {
        self.gpu_rt.free_sampler(self.handle.clone()).unwrap()
    }
}

/// An arc reference to a handle which is dropped at lifetime end
#[derive(Debug, Clone, PartialEq)]
pub struct SamplerHandle<A: Allocator = GPUAllocatorImpl> {
    inner: Arc<SamplerHandleInner<A>>,
}

impl<A: Allocator> SamplerHandle<A> {
    pub fn new(handle: Handle<resource::Sampler>, gpu_rt: GPUResourceTable<A>) -> Self {
        Self {
            inner: Arc::new(SamplerHandleInner { handle, gpu_rt }),
        }
    }

    /// Get the underlying handle
    pub fn get_handle(&self) -> &Handle<resource::Sampler> {
        &self.inner.handle
    }
}
