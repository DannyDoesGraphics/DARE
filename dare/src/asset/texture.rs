use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;

#[derive(Debug, Clone)]
pub struct Texture<A: Allocator> {
    pub image: containers::DeferredDeletionSlot<resource::Image<A>>,
    pub image_view: containers::DeferredDeletionSlot<resource::ImageView>,
}