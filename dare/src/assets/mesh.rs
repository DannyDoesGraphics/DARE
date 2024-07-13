use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::util::Slot;

use crate::{physics, render};
use crate::render::deferred_deletion::DeletionEntry;

/// Describes a mesh at a high level including streaming said mesh in and out of gpu memory
#[derive(Debug)]
pub struct Mesh<A: Allocator = GPUAllocatorImpl> {
    handle: render::GPUResource<Slot<DeletionEntry<render::Mesh<A>>>>,
    transform: physics::Transform,
    name: String,
}