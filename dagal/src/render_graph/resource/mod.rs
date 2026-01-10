pub mod buffer;
pub mod image;
pub mod physical;
mod virtual_resource_store;

use ash::vk;
use std::fmt::Debug;
use std::hash::Hash;
pub use virtual_resource_store::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ResourceId(pub u64);
impl ResourceId {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self(((id as u64) << 32) | (generation as u64))
    }
    
    pub fn id(&self) -> u32 {
        (self.0 >> 32) as u32
    }
    
    pub fn generation(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Axis {
    Absolute(u32),
    /// Relative, typically to the swapchain extent
    Relative(f32),
}
impl Eq for Axis {}
impl Hash for Axis {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Axis::Absolute(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Axis::Relative(v) => {
                1u8.hash(state);
                (v.to_bits()).hash(state);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Extent3D {
    pub width: Axis,
    pub height: Axis,
    pub depth: Axis,
}
impl Extent3D {
    pub fn contains_relative(&self) -> bool {
        matches!(self.width, Axis::Relative(_))
            || matches!(self.height, Axis::Relative(_))
            || matches!(self.depth, Axis::Relative(_))
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Extent2D {
    pub width: Axis,
    pub height: Axis,
}
impl Extent2D {
    pub fn contains_relative(&self) -> bool {
        matches!(self.width, Axis::Relative(_)) || matches!(self.height, Axis::Relative(_))
    }
}

/// Describes an access to a resource, either a buffer or an image.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum UseDeclaration {
    Buffer {
        resource: ResourceId,
        access: buffer::AccessFlag,
        span: std::ops::Range<u64>,
    },
    Image {
        resource: ResourceId,
        access: image::AccessFlag,
        subresource: image::ImageSubresourceRange,
    },
}


pub trait VirtualableResource: 'static {
    type Description: 'static + Debug;
    
    type Physical: 'static + Debug;
    
    type PhysicalStore: 'static + Debug;
}