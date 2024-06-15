use bitflags::bitflags;

use dagal::allocators::{Allocator, GPUAllocatorImpl};

use crate::render;

#[derive(Debug, Clone)]
pub struct Material<A: Allocator = GPUAllocatorImpl> {
    color_factor: glam::Vec4,
    albedo: Option<render::Texture<A>>,
    normal: Option<render::Texture<A>>,
}

bitflags! {
    /// Flags of textures which are enabled
    struct TextureFlags: u32 {
        const Albedo = 0b00000001;
        const Normal = 0b00000010;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CMaterial {
    color_factor: glam::Vec4,
    /// Bit flags of enabled textures
    texture_flags: u32,
    /// Get albedo
    albedo: u32,
    normal: u32,
}