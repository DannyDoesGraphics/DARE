use std::sync::Arc;

use anyhow::Result;
use bitflags::bitflags;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::pipelines::GraphicsPipeline;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::ImmediateSubmit;

use crate::render;

#[derive(Debug)]
pub struct Material<A: Allocator = GPUAllocatorImpl> {
    color_factor: glam::Vec4,
    albedo: Option<render::Texture<A>>,
    normal: Option<render::Texture<A>>,
    buffer: resource::Buffer<A>,

    pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
}

impl<A: Allocator> Material<A> {
    pub fn new(
        allocator: &mut ArcAllocator<A>,
        pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
        color_factor: glam::Vec4,
        albedo: Option<render::Texture<A>>,
        normal: Option<render::Texture<A>>,
        device: dagal::device::LogicalDevice,
    ) -> Result<Self> {
        let mut buffer =
            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: device.clone(),
                allocator,
                size: std::mem::size_of::<CMaterial>() as vk::DeviceSize,
                memory_type: MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
            })?;
        if let Some(debug_utils) = device.get_debug_utils() {
            buffer.set_name(debug_utils, "Material buffer")?;
        }
        Ok(Self {
            pipeline,
            color_factor,
            albedo,
            normal,
            buffer,
        })
    }

    pub fn upload_material(
        &mut self,
        immediate: &mut ImmediateSubmit,
        allocator: &mut ArcAllocator<A>,
    ) -> Result<()> {
        let mut texture_flags: TextureFlags = TextureFlags::empty();
        texture_flags |= self
            .albedo
            .as_ref()
            .map(|_| TextureFlags::Albedo)
            .unwrap_or(TextureFlags::empty());
        texture_flags |= self
            .normal
            .as_ref()
            .map(|_| TextureFlags::Normal)
            .unwrap_or(TextureFlags::empty());
        self.buffer.upload(
            immediate,
            allocator,
            &[CMaterial {
                texture_flags: texture_flags.bits(),
                color_factor: self.color_factor,
                albedo: self
                    .albedo
                    .as_ref()
                    .map(|tex| tex.get_image().id())
                    .unwrap_or(u32::MAX as u64) as u32,
                normal: self
                    .normal
                    .as_ref()
                    .map(|tex| tex.get_image().id())
                    .unwrap_or(u32::MAX as u64) as u32,
            }],
        )?;
        Ok(())
    }

    pub fn get_buffer(&self) -> &resource::Buffer<A> {
        &self.buffer
    }

    pub fn get_pipeline(&self) -> &Arc<render::pipeline::Pipeline<GraphicsPipeline>> {
        &self.pipeline
    }
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
    /// Bit flags of enabled textures
    texture_flags: u32,
    color_factor: glam::Vec4,
    albedo: u32,
    normal: u32,
}
