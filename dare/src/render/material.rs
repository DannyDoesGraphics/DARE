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

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub enum AlphaMode {
    Opaque,
    Blend,
    Mask(f32),
}

#[derive(Debug)]
pub struct Material2<A: Allocator> {
    color_factor: glam::Vec4,
    albedo: Option<render::Texture2<A>>,
    normal: Option<render::Texture2<A>>,
    buffer: resource::Buffer<A>,
    name: String,
    pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
    alpha_mode: AlphaMode,
}

#[derive(Debug)]
pub struct Material<A: Allocator = GPUAllocatorImpl> {
    color_factor: glam::Vec4,
    albedo: Option<render::Texture<A>>,
    normal: Option<render::Texture<A>>,
    buffer: resource::Buffer<A>,
    name: String,
    pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
    alpha_mode: AlphaMode,
}

impl<A: Allocator> Material<A> {
    pub fn new(
        allocator: &mut ArcAllocator<A>,
        pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
        color_factor: glam::Vec4,
        albedo: Option<render::Texture<A>>,
        normal: Option<render::Texture<A>>,
        name: String,
        device: dagal::device::LogicalDevice,
        alpha_mode: AlphaMode,
    ) -> Result<Self> {
        let buffer = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
            device: device.clone(),
            allocator,
            size: std::mem::size_of::<CMaterial>() as vk::DeviceSize,
            memory_type: MemoryLocation::GpuOnly,
            usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST,
        })?;
        Ok(Self {
            pipeline,
            color_factor,
            albedo,
            normal,
            buffer,
            name,
            alpha_mode,
        })
    }

    pub async fn upload_material(
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
        if let Some(debug_utils) = immediate.get_device().get_debug_utils() {
            self.buffer.set_name(
                debug_utils,
                format!("{}_material_buffer", self.name).as_str(),
            )?;
        }
        self.buffer
            .upload(
                immediate,
                allocator,
                &[CMaterial {
                    texture_flags: texture_flags.bits(),
                    color_factor: self.color_factor.to_array(),
                    albedo: self
                        .albedo
                        .as_ref()
                        .map(|tex| tex.get_image().id() as u32)
                        .unwrap_or(0),
                    albedo_sampler: self
                        .albedo
                        .as_ref()
                        .map(|tex| tex.get_sampler().id() as u32)
                        .unwrap_or(0),
                    normal: self
                        .normal
                        .as_ref()
                        .map(|tex| tex.get_image().id() as u32)
                        .unwrap_or(0),
                    normal_sampler: self
                        .normal
                        .as_ref()
                        .map(|tex| tex.get_sampler().id() as u32)
                        .unwrap_or(0),
                }],
            )
            .await?;
        Ok(())
    }

    pub fn get_buffer(&self) -> &resource::Buffer<A> {
        &self.buffer
    }

    pub fn get_pipeline(&self) -> &Arc<render::pipeline::Pipeline<GraphicsPipeline>> {
        &self.pipeline
    }

    pub fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
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
    color_factor: [f32; 4],
    albedo: u32,
    albedo_sampler: u32,
    normal: u32,
    normal_sampler: u32,
}
