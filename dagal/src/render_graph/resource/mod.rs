pub(crate) mod memory;
pub mod buffer;
mod physical;

use std::fmt::Debug;
use std::hash::Hash;
use ash::vk;

#[derive(Debug, PartialEq, Clone)]
pub enum Extent3D  {
    Absolute {
        width: u32,
        height: u32,
        depth: u32,
    },
    // Size that is relative typically to another resource such as the swapchain
    Relative {
        width_factor: f32,
        height_factor: f32,
        depth_factor: f32,
    }
}
impl Eq for Extent3D  {}
impl Hash for Extent3D {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Extent3D::Absolute { width, height, depth } => {
                0u8.hash(state);
                width.hash(state);
                height.hash(state);
                depth.hash(state);
            }
            Extent3D::Relative { width_factor, height_factor, depth_factor } => {
                1u8.hash(state);
                (width_factor.to_bits()).hash(state);
                (height_factor.to_bits()).hash(state);
                (depth_factor.to_bits()).hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceDescription {
    Buffer {
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        location: crate::allocators::MemoryLocation,
        /// Whether or not the buffer should persist across frames
        persistent: bool,
    },
    Image {
        format: vk::Format,
        /// Relative extents reference the swapchain extent
        extent: Extent3D,
        samples: u32,
        levels: u32,
        layers: u32,
        location: crate::allocators::MemoryLocation,
        /// Whether or not the image should persist across frames
        persistent: bool,
    }
}