use std::sync::Arc;

use anyhow::Result;
use bitflags::bitflags;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::free_list_allocator::Handle;
use dagal::util::ImmediateSubmit;

use crate::render;

/// Describes a surface which can be rendered to
#[derive(Debug)]
pub struct Surface<A: Allocator = GPUAllocatorImpl> {
    material: Arc<render::Material<A>>,
    vertex_buffer: Handle<resource::Buffer<A>>,
    index_buffer: Handle<resource::Buffer<A>>,
    normal_buffer: Option<Handle<resource::Buffer<A>>>,
    uv_buffer: Option<Handle<resource::Buffer<A>>>,
    buffer: resource::Buffer<A>,
    gpu_rt: GPUResourceTable<A>,
    index_count: u32,
    first_index: u32,
}

pub struct SurfaceBuilder<'a, A: Allocator> {
    pub gpu_rt: GPUResourceTable<A>,
    pub material: Arc<render::Material<A>>,
    pub allocator: &'a mut ArcAllocator<A>,
    pub immediate: &'a mut ImmediateSubmit,
    pub indices: Vec<u8>,
    pub vertices: Vec<u8>,
    pub normals: Option<Vec<u8>>,
    pub uv: Option<Vec<u8>>,
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
    fn pad_to_16_bytes(vec: &mut Vec<u8>) {
        let len = vec.len();
        let padding_len = (16 - (len % 16)) % 16;
        vec.extend(vec![0; padding_len]);
    }
    pub fn from_primitives(mut builder: SurfaceBuilder<A>) -> Result<Self> {
        // padding
        Self::pad_to_16_bytes(&mut builder.vertices);
        //Self::pad_to_16_bytes(&mut indices);

        let mut index_buffer = builder.gpu_rt.new_buffer(ResourceInput::ResourceCI(
            resource::BufferCreateInfo::NewEmptyBuffer {
                device: builder.gpu_rt.get_device().clone(),
                allocator: builder.allocator,
                size: std::mem::size_of_val(builder.indices.as_slice()) as vk::DeviceSize,
                memory_type: MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::INDEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
            },
        ))?;
        let device = builder.gpu_rt.get_device().clone();
        builder.gpu_rt.with_buffer_mut(&index_buffer, |buf| {
            device.clone().get_debug_utils().map_or(Ok(()), |debug_utils| buf.set_name(debug_utils, format!("{}_index_buffer", builder.name).as_str()))
        })??;
        let mut vertex_buffer = builder.gpu_rt.new_buffer(ResourceInput::ResourceCI(
            resource::BufferCreateInfo::NewEmptyBuffer {
                device: builder.gpu_rt.get_device().clone(),
                allocator: builder.allocator,
                size: std::mem::size_of_val(builder.vertices.as_slice()) as vk::DeviceSize,
                memory_type: MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
            },
        ))?;
        builder.gpu_rt.with_buffer_mut(&vertex_buffer, |buf| {
            device.clone().get_debug_utils().map_or(Ok(()), |debug_utils| buf.set_name(debug_utils, format!("{}_vertex_buffer", builder.name).as_str()))
        })??;
        let uv_buffer = builder.uv.and_then(|uv| {
            builder.gpu_rt
                   .new_buffer(ResourceInput::ResourceCI(
                       resource::BufferCreateInfo::NewEmptyBuffer {
                           device: builder.gpu_rt.get_device().clone(),
                           allocator: builder.allocator,
                           size: std::mem::size_of_val(uv.as_slice()) as vk::DeviceSize, // Use &uv to get the size of the value
                           memory_type: MemoryLocation::GpuOnly,
                           usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                               | vk::BufferUsageFlags::STORAGE_BUFFER
                               | vk::BufferUsageFlags::TRANSFER_DST,
                       },
                   ))
                   .ok()
                   .and_then(|buffer| {
                       builder.gpu_rt
                              .with_buffer_mut(&buffer, |buf| buf.upload(builder.immediate, builder.allocator, &uv))
                              .ok()
                              .map(|_| buffer)
                   })
        });
        for (handle, data) in [(&mut index_buffer, builder.indices.as_slice()), (&mut vertex_buffer, builder.vertices.as_slice())] {
            builder.gpu_rt
                   .with_buffer_mut(handle, |buffer| buffer.upload(builder.immediate, builder.allocator, data))??;
        }
        let buffer = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
            device: builder.gpu_rt.get_device().clone(),
            allocator: builder.allocator,
            size: std::mem::size_of::<CSurface>() as vk::DeviceSize,
            memory_type: MemoryLocation::GpuOnly,
            usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST,
        })?;

        Ok(Self {
            material: builder.material,
            vertex_buffer,
            index_buffer,
            normal_buffer: None,
            uv_buffer,
            buffer,
            gpu_rt: builder.gpu_rt,
            index_count: builder.total_indices,
            first_index: builder.first_index,
        })
    }

    /// Get a reference to the material used by the mesh
    pub fn material(&self) -> &Arc<render::Material<A>> {
        &self.material
    }

    pub fn upload(
        &mut self,
        immediate: &mut ImmediateSubmit,
        allocator: &mut ArcAllocator<A>,
        transform: glam::Mat4,
    ) -> Result<()> {
        let mut buffer_flags = SurfaceBufferFlags::empty();
        buffer_flags |= self.normal_buffer.as_ref().map_or_else(
            SurfaceBufferFlags::empty,
            |_| SurfaceBufferFlags::Normal,
        );
        buffer_flags |= self
            .uv_buffer
            .as_ref()
            .map_or_else(SurfaceBufferFlags::empty, |_| SurfaceBufferFlags::UV);
        if let Some(debug_utils) = self.gpu_rt.get_device().get_debug_utils() {
            self.buffer.set_name(debug_utils, &format!("{}_surface", self.buffer.address()))?;
        }
        let vertices = self.gpu_rt.get_bda(&self.vertex_buffer)?;
        println!("{}", vertices);
        self.buffer.upload(
            immediate,
            allocator,
            &[CSurface {
                material: self.material.get_buffer().address(),
                transform: glam::Mat4::IDENTITY.transpose().to_cols_array(),
                buffer_flags: buffer_flags.bits(),
                _padding: 0,
                vertices: self.gpu_rt.get_bda(&self.vertex_buffer)?,
                indices: self.gpu_rt.get_bda(&self.index_buffer)?,
                normals: self
                    .normal_buffer
                    .as_ref()
                    .and_then(|buf| self.gpu_rt.get_bda(buf).ok())
                    .unwrap_or(u64::MAX as vk::DeviceAddress),
                tangents: 0,
                uvs: self.uv_buffer.as_ref()
                         .and_then(|buf| self.gpu_rt.get_bda(buf).ok())
                         .unwrap_or(u64::MAX as vk::DeviceAddress)
            }],
        )
    }

    pub fn get_index_buffer(&self) -> &Handle<resource::Buffer<A>> {
        &self.index_buffer
    }

    pub fn get_buffer(&self) -> &resource::Buffer<A> {
        &self.buffer
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn first_index(&self) -> u32 {
        self.first_index
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SurfaceBufferFlags: u32 {
        const Normal = 0b0000_0001;
        const Tangent = 0b0000_0010;
        const UV = 0b0000_0100;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CSurface {
    pub material: u64,
    pub transform: [f32; 16],
    pub buffer_flags: u32,
    pub _padding: u32,
    pub vertices: u64,
    pub indices: u64,
    pub normals: u64,
    pub tangents: u64,
    pub uvs: u64,
}
