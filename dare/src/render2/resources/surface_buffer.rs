use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dare_containers::prelude as containers;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct RenderSurfaceBuffer<A: Allocator + 'static> {
    pub growable_buffer: dare::render::util::GrowableBuffer<A>,
    // hash of what was uploaded
    hash: u64,
}

impl<A: Allocator> RenderSurfaceBuffer<A> {
    pub fn new(growable_buffer: dare::render::util::GrowableBuffer<A>) -> Self {
        Self {
            growable_buffer,
            hash: 0,
        }
    }
}

/// Manages the render side of surfaces
#[derive(becs::Resource)]
pub struct RenderSurfaceManager {
    pub uploaded_hash: u64,
    pub mesh_container: containers::InsertionSortSlotMap<dare::engine::components::Surface>,
    pub surface_hashes: HashMap<
        dare::engine::components::Surface,
        containers::Slot<dare::engine::components::Surface>,
    >,
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
