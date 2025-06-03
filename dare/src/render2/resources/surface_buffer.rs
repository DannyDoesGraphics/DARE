use crate::prelude as dare;
use dagal::allocators::Allocator;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct RenderSurfaceBuffer<A: Allocator + 'static> {
    pub growable_buffer: dare::render::util::GrowableBuffer<A>,
}

impl<A: Allocator> RenderSurfaceBuffer<A> {
    pub fn new(growable_buffer: dare::render::util::GrowableBuffer<A>) -> Self {
        Self { growable_buffer }
    }
}

impl<A: Allocator> Deref for RenderSurfaceBuffer<A> {
    type Target = dare::render::util::GrowableBuffer<A>;

    fn deref(&self) -> &Self::Target {
        &self.growable_buffer
    }
}

impl<A: Allocator> DerefMut for RenderSurfaceBuffer<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.growable_buffer
    }
}
