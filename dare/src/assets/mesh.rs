use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};

use crate::render;

#[derive(Debug)]
pub struct Mesh<A: Allocator = GPUAllocatorImpl> {
    name: Option<String>,
    pub position: glam::Vec3,
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    surfaces: Vec<Arc<render::Surface<A>>>,
}

impl<A: Allocator> Mesh<A> {
    /// Get the underlying surfaces of a mesh
    pub fn get_surfaces(&self) -> &[Arc<render::Surface<A>>] {
        &self.surfaces
    }

    pub fn new(
        name: Option<String>,
        position: glam::Vec3,
        scale: glam::Vec3,
        rotation: glam::Quat,
        surfaces: Vec<Arc<render::Surface<A>>>,
    ) -> Self {
        Self {
            name,
            position,
            scale,
            rotation,
            surfaces,
        }
    }
}
