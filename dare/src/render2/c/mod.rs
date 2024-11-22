pub mod indirect_buffers;
#[allow(unused_imports)]
pub use indirect_buffers::*;

use std::hash::{Hash, Hasher};
use crate::prelude as dare;
use bitflags::bitflags;
use dagal::allocators::{Allocator, GPUAllocatorImpl};

bitflags! {
    #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
    pub struct MaterialFlags: u32 {
        const NONE = 0;
        const ALBEDO = 1 << 0;
        const NORMAL = 1 << 1;
    }
}

/// Underlying C representation of a surface
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CSurface {
    pub material: u64,
    pub bit_flag: u32,
    pub _padding: u32,
    pub positions: u64,
    pub indices: u64,
    pub normals: u64,
    pub tangents: u64,
    pub uv: u64,
    pub transform: [f32; 16],
}

impl Hash for CSurface {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        self.bit_flag.hash(state);
        self.positions.hash(state);
        self.indices.hash(state);
        self.normals.hash(state);
        self.tangents.hash(state);
        self.uv.hash(state);
        //let _ = self.transform.iter().map(|i| (*i).hash(state));
    }
}

impl CSurface {
    pub fn from_surface(asset_server: &dare::render::render_assets::RenderAssetsStorage<dare::render::components::RenderBuffer<GPUAllocatorImpl>>, surface: dare::engine::components::Surface, transform: &glam::Mat4) -> Option<Self> {
        Some(Self {
            material: 0,
            bit_flag: 0,
            _padding: 0,
            positions: asset_server.get_bda(&surface.vertex_buffer.id())?,
            indices: asset_server.get_bda(&surface.index_buffer.id())?,
            normals: surface.normal_buffer.as_ref().map(|buffer| asset_server.get_bda(&buffer.id())).flatten().unwrap_or(0),
            tangents: surface.tangent_buffer.as_ref().map(|buffer| asset_server.get_bda(&buffer.id())).flatten().unwrap_or(0),
            uv: 0,
            transform: transform.to_cols_array(),
        })
    }
}

/// Underlying C mesh representation of a mesh
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CMesh {
    pub material: u64,
    pub bit_flag: u32,
    pub positions: u64,
    pub indices: u64,
    pub normals: u64,
    pub tangents: u64,
    pub uv: u64,
    pub transform: [f32; 16],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CMaterial {
    pub bit_flag: u32,
    pub color_factor: [f32; 4],
    pub albedo_texture_id: u32,
    pub albedo_sampler_id: u32,
    pub normal_texture_id: u32,
    pub normal_sampler_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CPushConstant {
    pub transform: [f32; 16],
    pub vertex_buffer: u64,
}
