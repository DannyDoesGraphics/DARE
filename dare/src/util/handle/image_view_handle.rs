use std::sync::Arc;

use dagal::allocators::Allocator;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

#[derive(Debug)]
struct ImageViewHandleInner<A: Allocator> {
    handle: Handle<resource::ImageView>,
    gpu_rt: GPUResourceTable<A>,
}

impl<A: Allocator> Drop for ImageViewHandleInner<A> {
    fn drop(&mut self) {
        self.gpu_rt.free_image_view(self.handle.clone()).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct ImageViewHandle<A: Allocator> {
    handle: Arc<ImageViewHandleInner<A>>,
}

impl<A: Allocator> ImageViewHandle<A> {
    pub fn new(handle: Handle<resource::ImageView>, gpu_rt: GPUResourceTable<A>) -> Self {
        Self {
            handle: Arc::new(ImageViewHandleInner { handle, gpu_rt }),
        }
    }

    pub fn get_handle(&self) -> &Handle<resource::ImageView> {
        &self.handle.handle
    }

    pub fn get_gpu_rt(&self) -> &GPUResourceTable<A> {
        &self.handle.gpu_rt
    }
}
