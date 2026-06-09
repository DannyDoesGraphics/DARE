use dagal::allocators::Allocator;

#[derive(Debug)]
pub struct SwapchainFrame<A: Allocator> {
    pub image_view: dagal::resource::ImageView,
    pub image: dagal::resource::Image<A>,
}

#[derive(Debug)]
pub struct Frame {
    /// Used by CPU to know when rendering is done
    pub render_fence: dagal::sync::Fence,
    /// Signaled when the swapchain image is available for rendering
    pub swapchain_semaphore: dagal::sync::BinarySemaphore,
    pub command_pool: dagal::command::CommandPool,
    pub command_buffer: dagal::command::CommandBuffer,
}
