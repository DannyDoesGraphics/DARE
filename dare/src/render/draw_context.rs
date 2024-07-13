use std::hash::{DefaultHasher, Hash, Hasher};

use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::util::Slot;

use crate::render;
use crate::render::deferred_deletion::DeletionEntry;

/// Contains a context necessary for knowing that state of the scene for rendering
#[derive(Debug)]
pub struct DrawContext<A: Allocator = GPUAllocatorImpl> {
    pub surfaces: Vec<Slot<DeletionEntry<render::WeakMesh<A>>>>,
    pub last_draw_hash: u64,
    pub last_draw_hash_ordered: u64,
    pub difference: bool,
}

impl<A: Allocator> Default for DrawContext<A> {
    fn default() -> Self {
        Self {
            surfaces: Vec::new(),
            last_draw_hash: 0,
            last_draw_hash_ordered: 0,
            difference: false,
        }
    }
}

impl<A: Allocator> DrawContext<A> {
    pub fn hash_surfaces(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.surfaces.hash(&mut hasher);
        hasher.finish()
    }

    pub fn hash_surfaces_ordered(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (index, surface) in self.surfaces.iter().enumerate() {
            index.hash(&mut hasher);
            surface.hash(&mut hasher);
        }
        hasher.finish()
    }
}