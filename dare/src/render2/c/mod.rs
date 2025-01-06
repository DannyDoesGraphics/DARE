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
        buffers: &dare::render::render_assets::storage::RenderAssetManagerStorage<
            dare::render::components::RenderBuffer<GPUAllocatorImpl>,
        >,
        surface: dare::engine::components::Surface,
    ) -> Option<Self> {
        Some(Self {
            material: 1,
            bit_flag: 2,
            _padding: 0,
            positions: buffers.get_bda_from_asset_handle(&surface.vertex_buffer)?,
            indices: buffers.get_bda_from_asset_handle(&surface.index_buffer)?,
            normals: surface
                .normal_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda_from_asset_handle(buffer))
                .unwrap_or(Some(0))?,
            tangents: surface
                .tangent_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda_from_asset_handle(buffer))
                .unwrap_or(Some(0))?,
            uv: 0,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CMaterial {
    pub bit_flag: u32,
    pub _padding: u32,
    pub color_factor: [f32; 4],
    pub albedo_texture_id: u32,
    pub albedo_sampler_id: u32,
    pub normal_texture_id: u32,
    pub normal_sampler_id: u32,
}
impl CMaterial {
    pub fn from_material(material: dare::engine::components::Material) -> Option<Self> {
        Some(Self {
            bit_flag: 0,
            _padding: 0,
            color_factor: material.albedo_factor.to_array(), 
            albedo_texture_id: 0,
            albedo_sampler_id: 0,
            normal_texture_id: 0,
            normal_sampler_id: 0,
        })
    }
}
unsafe impl Zeroable for CMaterial {}
unsafe impl Pod for CMaterial {}


#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CPushConstant {
    pub transform: [f32; 16],
    pub instanced_surface_info: u64,
    pub surface_infos: u64,
    pub transforms: u64,
    pub draw_id: u64,
}
unsafe impl Zeroable for CPushConstant {}
unsafe impl Pod for CPushConstant {}