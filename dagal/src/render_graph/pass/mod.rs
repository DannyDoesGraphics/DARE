//! This module contains all the render passes used in the render graph.
//!
//! # Builder
//! The builder pattern is used to build precompiled passes
//!
//! # Precompiled passes
//! Precompiled passes which passes which have yet to be compiled by the render graph
//!
//! # Compiled passes
//! Passes which have been compiled entirely

pub mod compute_pass;
pub mod descriptor;
pub mod pass_storage;
use ash::vk;
use derivative::Derivative;
pub use pass_storage::*;

#[derive(Debug)]
pub struct PassContext<A: crate::allocators::Allocator = crate::DefaultAllocator> {
    pub command_buffer: crate::command::CommandBuffer,
    pub device: crate::device::LogicalDevice,
    pub allocator: A,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct PassId(pub u32);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PassKind {
    Graphics {},
    Compute { extent: super::resource::Extent3D },
    Raytracing {},
}

#[derive(Debug, PartialEq, Clone)]
pub struct ColorAttachment {
    pub resource: super::resource::ResourceId,
    pub subresource: super::resource::image::ImageSubresourceRange,
    pub load: vk::AttachmentLoadOp,
    pub store: vk::AttachmentStoreOp,
    pub clear_value: [f32; 4],
}
impl Eq for ColorAttachment {}
impl std::hash::Hash for ColorAttachment {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.resource.hash(state);
        self.subresource.hash(state);
        self.load.hash(state);
        self.store.hash(state);
        for v in &self.clear_value {
            (v.to_bits()).hash(state);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DepthAttachment {
    pub resource: super::resource::ResourceId,
    pub subresource: super::resource::image::ImageSubresourceRange,
    pub depth_load: vk::AttachmentLoadOp,
    pub depth_store: vk::AttachmentStoreOp,
    pub depth_clear: [f32; 4],
    pub stencil_load: vk::AttachmentLoadOp,
    pub stencil_store: vk::AttachmentStoreOp,
    pub stencil_clear: [f32; 4],
}
impl Eq for DepthAttachment {}
impl std::hash::Hash for DepthAttachment {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.resource.hash(state);
        self.subresource.hash(state);
        self.depth_load.hash(state);
        self.depth_store.hash(state);
        for v in &self.depth_clear {
            (v.to_bits()).hash(state);
        }
        self.stencil_load.hash(state);
        self.stencil_store.hash(state);
        for v in &self.stencil_clear {
            (v.to_bits()).hash(state);
        }
    }
}

#[derive(Derivative)]
#[derivative(Hash, Debug, PartialEq)]
pub struct PassDescription<A: crate::allocators::Allocator = crate::DefaultAllocator> {
    pub name: String,
    pub kind: PassKind,
    pub reads: Vec<(super::resource::ResourceId, super::resource::UseDeclaration)>,
    pub writes: Vec<(super::resource::ResourceId, super::resource::UseDeclaration)>,
    pub root: bool,
    #[derivative(Hash = "ignore", PartialEq = "ignore", Debug = "ignore")]
    pub execute: Box<dyn Fn(PassContext<A>)>,
}
impl<A: crate::allocators::Allocator> Eq for PassDescription<A> {}

impl<A: crate::allocators::Allocator> PassDescription<A> {
    pub fn compute(name: String, root: bool, extent: super::resource::Extent3D) -> Self {
        Self {
            name,
            kind: PassKind::Compute { extent },
            reads: Vec::new(),
            writes: Vec::new(),
            root,
            execute: Box::new(|_| {}),
        }
    }

    pub fn execute<F: Fn(PassContext<A>) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.execute = Box::new(f);
        self
    }

    pub fn read(
        mut self,
        resource: super::resource::ResourceId,
        access: super::resource::UseDeclaration,
    ) -> Self {
        self.reads.push((resource, access));
        self
    }

    pub fn write(
        mut self,
        resource: super::resource::ResourceId,
        access: super::resource::UseDeclaration,
    ) -> Self {
        self.writes.push((resource, access));
        self
    }
}
