use dagal::allocators::Allocator;

#[derive(Debug)]
pub struct Frame<A: Allocator> {
    pub semaphore: dagal::sync::BinarySemaphore,
    pub image_view: dagal::resource::ImageView,
    pub image: dagal::resource::Image<A>,
}
