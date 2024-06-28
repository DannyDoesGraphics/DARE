use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};

/// Holds references to draw surfaces
#[derive(Debug)]
pub struct DrawSurface<A: Allocator = GPUAllocatorImpl> {
    pub surface: Arc<crate::render::Surface<A>>,
    pub local_transform: glam::Mat4,
}

/// Contains a context necessary for knowing that state of the scene for rendering
#[derive(Debug)]
pub struct DrawContext<A: Allocator = GPUAllocatorImpl> {
    pub surfaces: Vec<DrawSurface<A>>,
}

impl<A: Allocator> Default for DrawContext<A> {
    fn default() -> Self {
        Self {
            surfaces: Vec::new(),
        }
    }
}
