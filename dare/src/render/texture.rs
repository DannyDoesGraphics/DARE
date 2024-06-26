use dagal::allocators::Allocator;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::util::free_list_allocator::Handle;

use crate::util::handle;

#[derive(Debug, Clone)]
pub struct Texture<A: Allocator> {
    image: handle::ImageHandle<A>,
    sampler: handle::SamplerHandle<A>,
}

impl<A: Allocator> Texture<A> {
    pub fn new(
        image: Handle<resource::Image<A>>,
        sampler: Handle<resource::Sampler>,
        gpu_rt: GPUResourceTable<A>,
    ) -> Self {
        Self {
            image: handle::ImageHandle::new(image, gpu_rt.clone()),
            sampler: handle::SamplerHandle::new(sampler, gpu_rt.clone()),
        }
    }

    pub fn get_image(&self) -> &Handle<resource::Image<A>> {
        &self.image.get_handle()
    }

    pub fn get_sampler(&self) -> &Handle<resource::Sampler> {
        &self.sampler.get_handle()
    }
}
