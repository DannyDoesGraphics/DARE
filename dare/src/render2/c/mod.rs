pub mod indirect_buffers;
#[allow(unused_imports)]
pub use indirect_buffers::*;

use crate::prelude as dare;
use crate::render2::physical_resource;
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use std::hash::{Hash, Hasher};

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
    pub transform: [f32; 16],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub material: u64,
    pub bit_flag: u32,
    pub index_count: u32,
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
        buffers: &mut physical_resource::PhysicalResourceStorage<
            physical_resource::RenderBuffer<GPUAllocatorImpl>,
        >,
        surface: &dare::engine::components::Surface,
        transform: &dare::physics::components::Transform,
        bounding_box: &dare::render::components::BoundingBox,
    ) -> Option<Self> {
        let positions = buffers.get_bda(&surface.vertex_buffer)?;
        let indices = buffers.get_bda(&surface.index_buffer)?;
        Some(Self {
            transform: transform.get_transform_matrix().transpose().to_cols_array(),
            min: bounding_box.min.to_array(),
            max: bounding_box.max.to_array(),
            material: 1,
            bit_flag: 2,
            index_count: surface.index_count as u32,
            positions,
            indices,
            normals: surface
                .normal_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda(buffer))
                .unwrap_or(Some(0))?,
            tangents: surface
                .tangent_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda(buffer))
                .unwrap_or(Some(0))?,
            uv: 0,
        })
    }

    /// Similar to [`Self::from_surface`], but will fill empty with 0
    pub fn from_surface_zero(
        buffers: &mut physical_resource::PhysicalResourceStorage<
            physical_resource::RenderBuffer<GPUAllocatorImpl>,
        >,
        surface: &dare::engine::components::Surface,
        transform: &dare::physics::components::Transform,
        bounding_box: &dare::render::components::BoundingBox,
    ) -> Self {
        let positions = buffers.get_bda(&surface.vertex_buffer).unwrap_or(0);
        let indices = buffers.get_bda(&surface.index_buffer).unwrap_or(0);
        Self {
            transform: transform.get_transform_matrix().transpose().to_cols_array(),
            min: bounding_box.min.to_array(),
            max: bounding_box.max.to_array(),
            material: 1,
            bit_flag: 2,
            index_count: surface.index_count as u32,
            positions,
            indices,
            normals: surface
                .normal_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda(buffer))
                .flatten()
                .unwrap_or(0),
            tangents: surface
                .tangent_buffer
                .as_ref()
                .map(|buffer| buffers.get_bda(buffer))
                .flatten()
                .unwrap_or(0),
            uv: 0,
        }
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
    pub fn from_material<A: Allocator>(
        transfer_pool: dare::render::util::TransferPool<A>,
        textures: &mut physical_resource::PhysicalResourceStorage<
            physical_resource::RenderImage<A>,
        >,
        material: dare::engine::components::Material,
    ) -> Option<Self> {
        let mut bit_flag = MaterialFlags::NONE;
        let albedo_texture_id = material
            .albedo_texture
            .map(|t| {
                textures
                    .resolve_virtual_resource(&t.asset_handle)
                    .map(|h| h.uid as u32)
            })
            .flatten();

        if albedo_texture_id.is_some() {
            bit_flag |= MaterialFlags::ALBEDO;
        };

        Some(Self {
            bit_flag: bit_flag.bits(),
            _padding: 128,
            color_factor: material.albedo_factor.to_array(),
            albedo_texture_id: albedo_texture_id.unwrap_or(0),
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
    pub draw_id: u64,
}
unsafe impl Zeroable for CPushConstant {}
unsafe impl Pod for CPushConstant {}
