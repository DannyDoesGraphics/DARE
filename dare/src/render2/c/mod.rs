pub mod indirect_buffers;
#[allow(unused_imports)]
pub use indirect_buffers::*;

use crate::prelude as dare;
use bitflags::bitflags;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use std::hash::{Hash, Hasher};
use bytemuck::{Pod, Zeroable};

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
}

unsafe impl Zeroable for CSurface {}
unsafe impl Pod for CSurface {}

impl Hash for CSurface {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        self.bit_flag.hash(state);
        self.positions.hash(state);
        self.indices.hash(state);
        self.normals.hash(state);
        self.tangents.hash(state);
        self.uv.hash(state);
    }
}

impl CSurface {
    pub fn from_surface(
        asset_server: &dare::render::render_assets::RenderAssetsStorage<
            dare::render::components::RenderBuffer<GPUAllocatorImpl>,
        >,
        surface: dare::engine::components::Surface,
    ) -> Option<Self> {
        Some(Self {
            material: 1,
            bit_flag: 2,
            _padding: 0,
            positions: asset_server.get_bda(&surface.vertex_buffer.id())?,
            indices: asset_server.get_bda(&surface.index_buffer.id())?,
            normals: surface
                .normal_buffer
                .as_ref()
                .map(|buffer| asset_server.get_bda(&buffer.id()))
                .unwrap_or(Some(0))?,
            tangents: surface
                .tangent_buffer
                .as_ref()
                .map(|buffer| asset_server.get_bda(&buffer.id()))
                .unwrap_or(Some(0))?,
            uv: 0,
        })
    }
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
    pub instanced_surface_info: u64,
    pub draw_id: u64,
}
