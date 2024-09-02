use dagal::allocators::Allocator;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;
use std::sync::Arc;

use crate::util::handle;

#[derive(Debug, Clone)]
pub struct Texture2<A: Allocator> {
    image: Arc<resource::Image<A>>,
    image_view: Arc<resource::ImageView>,
    sampler: Arc<resource::Sampler>,
}

#[derive(Debug, Clone)]
pub struct Texture<A: Allocator> {
    image: handle::ImageHandle<A>,
    image_view: handle::ImageViewHandle<A>,
    sampler: handle::SamplerHandle<A>,
}

impl<A: Allocator> Texture<A> {
    pub fn new(
        image: Handle<resource::Image<A>>,
        image_view: Handle<resource::ImageView>,
        sampler: Handle<resource::Sampler>,
        gpu_rt: GPUResourceTable<A>,
    ) -> Self {
        Self {
            image: handle::ImageHandle::new(image, gpu_rt.clone()),
            image_view: handle::ImageViewHandle::new(image_view, gpu_rt.clone()),
            sampler: handle::SamplerHandle::new(sampler, gpu_rt.clone()),
        }
    }

    pub fn from_handles(
        image: handle::ImageHandle<A>,
        image_view: handle::ImageViewHandle<A>,
        sampler: handle::SamplerHandle<A>,
    ) -> Self {
        Self {
            image,
            image_view,
            sampler,
        }
    }

    pub fn get_image(&self) -> &Handle<resource::Image<A>> {
        &self.image.get_handle()
    }

    pub fn get_image_view(&self) -> &Handle<resource::ImageView> {
        &self.image_view.get_handle()
    }

    pub fn get_sampler(&self) -> &Handle<resource::Sampler> {
        &self.sampler.get_handle()
    }
}
