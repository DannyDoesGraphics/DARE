use std::sync::Arc;

use anyhow::Result;
use bevy_ecs::prelude::*;
use bitflags::bitflags;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::free_list_allocator::Handle;

use crate::render;
use crate::traits::ReprC;

/// Describes a surface which can be rendered to
#[derive(Debug, Component)]
pub struct Surface<A: Allocator = GPUAllocatorImpl> {
    material: Arc<render::Material<A>>,
    vertex_buffer: Handle<resource::Buffer<A>>,
    index_buffer: Handle<resource::Buffer<A>>,
    normal_buffer: Option<Handle<resource::Buffer<A>>>,
    uv_buffer: Option<Handle<resource::Buffer<A>>>,
    gpu_rt: GPUResourceTable<A>,
    vertex_count: u32,
    index_count: u32,
    first_index: u32,
    name: Option<String>,
}

pub struct Surface2<A: Allocator> {
    material: Arc<render::Material<A>>,
}

pub struct SurfaceHandleBuilder<'a, A: Allocator> {
    pub gpu_rt: GPUResourceTable<A>,
    pub allocator: &'a mut ArcAllocator<A>,
    pub material: Arc<render::Material<A>>,
    pub indices: Handle<resource::Buffer<A>>,
    pub positions: Handle<resource::Buffer<A>>,
    pub normals: Option<Handle<resource::Buffer<A>>>,
    pub uv: Option<Handle<resource::Buffer<A>>>,
    pub total_vertices: u32,
    pub total_indices: u32,
    pub first_index: u32,
    pub name: &'a str,
}

impl<A: Allocator> Drop for Surface<A> {
    fn drop(&mut self) {
        self.gpu_rt.free_buffer(self.vertex_buffer.clone()).unwrap();
        self.gpu_rt.free_buffer(self.index_buffer.clone()).unwrap();
        for buf in [self.normal_buffer.as_ref(), self.uv_buffer.as_ref()]
            .into_iter()
            .flatten()
        {
            self.gpu_rt.free_buffer(buf.clone()).unwrap();
        }
    }
}

impl<A: Allocator> Surface<A> {
    pub fn from_handles(mut builder: SurfaceHandleBuilder<A>) -> Result<Self> {
        if let Some(debug) = builder.gpu_rt.get_device().clone().get_debug_utils() {
            builder
                .gpu_rt
                .with_buffer_mut(&builder.positions, |buffer| {
                    buffer.set_name(debug, format!("{}_position", builder.name).as_str())
                })??;
            builder
                .gpu_rt
                .with_buffer_mut(&builder.indices, |buffer| {
                    buffer.set_name(debug, format!("{}_indices", builder.name).as_str())
                })??;
            if let Some(normal) = builder.normals.as_ref() {
                builder.gpu_rt.with_buffer_mut(normal, |buffer| {
                    buffer.set_name(debug, format!("{}_uv", builder.name).as_str())
                })??;
            }
            if let Some(uv) = builder.uv.as_ref() {
                builder.gpu_rt.with_buffer_mut(uv, |buffer| {
                    buffer.set_name(debug, format!("{}_uv", builder.name).as_str())
                })??;
            }
        }
        Ok(Self {
            material: builder.material,
            vertex_buffer: builder.positions,
            index_buffer: builder.indices,
            normal_buffer: builder.normals,
            uv_buffer: builder.uv,
            gpu_rt: builder.gpu_rt,
            vertex_count: builder.total_vertices,
            index_count: builder.total_indices,
            first_index: builder.first_index,
            name: Some(builder.name.to_string()),
        })
    }

    /// Get a reference to the material used by the mesh
    pub fn material(&self) -> &Arc<render::Material<A>> {
        &self.material
    }

    pub fn get_gpu_rt(&self) -> &GPUResourceTable<A> {
        &self.gpu_rt
    }

    pub fn get_vertex_buffer(&self) -> &Handle<resource::Buffer<A>> {
        &self.vertex_buffer
    }

    pub fn get_index_buffer(&self) -> &Handle<resource::Buffer<A>> {
        &self.index_buffer
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn first_index(&self) -> u32 {
        self.first_index
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

impl<A: Allocator> ReprC for Surface<A> {
    type CType = CSurface;

    fn as_c(&self) -> CSurface {
        let mut buffer_flags = SurfaceBufferFlags::empty();
        buffer_flags |= self
            .normal_buffer
            .as_ref()
            .map_or_else(SurfaceBufferFlags::empty, |_| SurfaceBufferFlags::Normal);
        buffer_flags |= self
            .uv_buffer
            .as_ref()
            .map_or_else(SurfaceBufferFlags::empty, |_| SurfaceBufferFlags::UV);
        CSurface {
            material: self.material.get_buffer().address(),
            buffer_flags: buffer_flags.bits(),
            vertices: self.gpu_rt.get_bda(&self.vertex_buffer).unwrap(),
            indices: self.gpu_rt.get_bda(&self.index_buffer).unwrap(),
            normals: self
                .normal_buffer
                .as_ref()
                .and_then(|buffer| self.gpu_rt.get_bda(buffer).ok())
                .unwrap_or_default(),
            tangents: 0,
            uvs: self
                .uv_buffer
                .as_ref()
                .and_then(|buffer| self.gpu_rt.get_bda(buffer).ok())
                .unwrap_or_default(),
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SurfaceBufferFlags: u32 {
        const Normal =  0b0000_0001;
        const Tangent = 0b0000_0010;
        const UV =      0b0000_0100;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct CSurface {
    pub material: u64,
    pub buffer_flags: u32,
    pub vertices: u64,
    pub indices: u64,
    pub normals: u64,
    pub tangents: u64,
    pub uvs: u64,
}
