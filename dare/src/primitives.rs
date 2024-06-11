use std::mem;
use std::sync::Arc;

use dagal::allocators::{ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::{Pipeline, PipelineBuilder};
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::shader::ShaderCompiler;
use dagal::util::free_list_allocator::Handle;
use dagal::util::ImmediateSubmit;

use crate::{AllocatedImage, RenderContext};

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub uv_x: f32,
    pub normal: glam::Vec3,
    pub uv_y: f32,
    pub color: glam::Vec4,
}

pub struct GPUMeshBuffer {
    pub index_buffer: resource::Buffer<GPUAllocatorImpl>,
    pub vertex_buffer: Handle<resource::Buffer<GPUAllocatorImpl>>,
    gpu_rt: GPUResourceTable<GPUAllocatorImpl>,
}

impl Drop for GPUMeshBuffer {
    fn drop(&mut self) {
        self.gpu_rt.free_buffer(self.vertex_buffer.clone()).unwrap()
    }
}

impl GPUMeshBuffer {
    pub fn new(
        allocator: &mut dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
        immediate: &mut dagal::util::ImmediateSubmit,
        gpu_resource_table: &mut GPUResourceTable<GPUAllocatorImpl>,
        indices: &[u32],
        vertices: &[Vertex],
        name: Option<String>,
    ) -> Self {
        let mut index_buffer =
            resource::Buffer::<GPUAllocatorImpl>::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: immediate.get_device().clone(),
                allocator,
                size: mem::size_of_val(indices) as vk::DeviceSize,
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::INDEX_BUFFER,
            })
                .unwrap();
        let vertex_buffer_handle = gpu_resource_table
            .new_buffer(ResourceInput::ResourceCI(
                resource::BufferCreateInfo::NewEmptyBuffer {
                    device: immediate.get_device().clone(),
                    allocator,
                    size: mem::size_of_val(vertices) as vk::DeviceSize,
                    memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                    usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                }))
            .unwrap();
        index_buffer.upload(immediate, allocator, indices).unwrap(); // fuck it lol
        gpu_resource_table
            .with_buffer_mut(&vertex_buffer_handle, |buffer| {
                buffer
                    .upload::<Vertex>(immediate, allocator, vertices)
                    .unwrap();
                if let Some(name) = name.as_deref() {
                    let vertex_name = {
                        let mut n = name.to_string();
                        n.push_str(" vertex");
                        n
                    };
                    if let Some(debug_utils) = immediate.get_device().get_debug_utils() {
                        buffer.set_name(debug_utils, vertex_name.as_str()).unwrap()
                    };
                }
            })
            .unwrap();

        if let Some(debug_utils) = immediate.get_device().get_debug_utils() {
            if let Some(name) = name {
                let index_name = {
                    let mut n = name.clone();
                    n.push_str(" index");
                    n
                };
                index_buffer
                    .set_name(debug_utils, index_name.as_str())
                    .unwrap();
            }
        }
        Self {
            index_buffer,
            vertex_buffer: vertex_buffer_handle,
            gpu_rt: gpu_resource_table.clone(),
        }
    }
}

#[repr(C)]
pub struct GPUDrawPushConstants {
    pub world_matrix: glam::Mat4,
    pub vertex_buffer_id: u32,
}

#[derive(Debug, Clone, Copy, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct GeometrySurface {
    pub start_index: u32,
    pub count: u32,
}

pub struct MeshAsset {
    pub name: String,

    pub surfaces: Vec<GeometrySurface>,
    pub mesh_buffers: GPUMeshBuffer,
}

#[derive(Debug, Copy, Clone)]
pub enum MaterialPass {
    MainColor,
    Transparent,
    Other,
}

/// Information regarding the scene itself
#[repr(C)]
pub struct SceneData {
    view: glam::Mat4,
    proj: glam::Mat4,
    view_proj: glam::Mat4,
    ambient_color: glam::Vec4,
    sunlight_direction: glam::Vec4,
    sunlight_color: glam::Vec4,
}

/// Information about the mesh itself
pub struct MeshData {
    material: Handle<resource::Buffer<GPUAllocatorImpl>>,
}

/// Any information regarding the material of an object
pub struct MaterialInstance {
    pub color_factors: glam::Vec4,
    pub metal_rough_factors: glam::Vec4,

    pub color_image: AllocatedImage<GPUAllocatorImpl>,
    pub color_image_sampler: Handle<resource::Sampler>,

    pub metal_rough_image: AllocatedImage<GPUAllocatorImpl>,
    pub metal_rough_image_sampler: Handle<resource::Sampler>,

    pub data_buffer: Handle<resource::Buffer<GPUAllocatorImpl>>,
    pub gpu_rt: GPUResourceTable<GPUAllocatorImpl>,
}

impl Drop for MaterialInstance {
    fn drop(&mut self) {
        if self.color_image_sampler != self.metal_rough_image_sampler {
            self.gpu_rt.free_sampler(self.color_image_sampler.clone()).unwrap();
            self.gpu_rt.free_sampler(self.color_image_sampler.clone()).unwrap();
        }
    }
}

pub struct MaterialResources {
    pub color_image: AllocatedImage<GPUAllocatorImpl>,
    pub color_sampler: Handle<resource::Sampler>,
    pub metal_rough_image: AllocatedImage<GPUAllocatorImpl>,
    pub metal_sampler: Handle<resource::Sampler>,
}

pub struct GLTF_Metallic_Roughness_inner {
    pub transparent_pipeline: dagal::pipelines::GraphicsPipeline,
    pub opaque_pipeline: dagal::pipelines::GraphicsPipeline,
    pub layout: dagal::pipelines::PipelineLayout,
}

#[derive(Clone)]
pub struct GLTF_Metallic_Roughness {
    pub inner: Arc<GLTF_Metallic_Roughness_inner>,
}

#[repr(C)]
pub struct GPUMaterialPushConstants {
    material_index: u32,
    render_matrix: glam::Mat4,
}

impl GLTF_Metallic_Roughness {
    pub fn new(render_context: &mut RenderContext) -> Self {
        let layout = dagal::pipelines::PipelineLayoutBuilder::default()
            .push_bindless_gpu_resource_table(&render_context.gpu_resource_table)
            .push_push_constant_struct::<GPUMaterialPushConstants>(
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build(
                render_context.device.clone(),
                vk::PipelineLayoutCreateFlags::empty(),
            )
            .unwrap();
        let pipeline_builder = dagal::pipelines::GraphicsPipelineBuilder::default()
            .replace_layout(layout.handle())
            .set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .set_polygon_mode(vk::PolygonMode::FILL)
            .set_multisampling_none()
            .disable_blending()
            .enable_depth_test(vk::TRUE, vk::CompareOp::GREATER_OR_EQUAL)
            .set_depth_format(render_context.depth_image.as_ref().unwrap().format())
            .set_color_attachment(render_context.draw_image.as_ref().unwrap().format());

        let shaderc_compiler = dagal::shader::ShaderCCompiler::new();
        let opaque_pipeline = pipeline_builder
            .clone()
            .replace_shader_from_source_file(
                render_context.device.clone(),
                &shaderc_compiler,
                std::path::PathBuf::from("./dare/shaders/mesh.vert"),
                vk::ShaderStageFlags::VERTEX,
            )
            .unwrap()
            .replace_shader_from_source_file(
                render_context.device.clone(),
                &shaderc_compiler,
                std::path::PathBuf::from("./dare/shaders/mesh.frag"),
                vk::ShaderStageFlags::FRAGMENT,
            )
            .unwrap()
            .build(render_context.device.clone())
            .unwrap();
        let transparent_pipeline = pipeline_builder
            .clone()
            .replace_shader_from_source_file(
                render_context.device.clone(),
                &shaderc_compiler,
                std::path::PathBuf::from("./dare/shaders/mesh.vert"),
                vk::ShaderStageFlags::VERTEX,
            )
            .unwrap()
            .replace_shader_from_source_file(
                render_context.device.clone(),
                &shaderc_compiler,
                std::path::PathBuf::from("./dare/shaders/mesh.frag"),
                vk::ShaderStageFlags::FRAGMENT,
            )
            .unwrap()
            .enable_blending_additive()
            .enable_depth_test(vk::FALSE, vk::CompareOp::GREATER_OR_EQUAL)
            .build(render_context.device.clone())
            .unwrap();
        Self {
            inner: Arc::new(GLTF_Metallic_Roughness_inner {
                transparent_pipeline,
                opaque_pipeline,
                layout,
            }),
        }
    }

    pub fn write_material(&self,
                          gpu_rt: &mut GPUResourceTable<GPUAllocatorImpl>,
                          immediate: &mut ImmediateSubmit,
                          allocator: &mut ArcAllocator<GPUAllocatorImpl>,
                          pass: MaterialPass,
                          resources: MaterialResources) -> MaterialInstance {
        let data_buffer = gpu_rt.new_buffer(
            ResourceInput::ResourceCI(resource::BufferCreateInfo::NewEmptyBuffer {
                device: gpu_rt.get_device().clone(),
                allocator,
                size: mem::size_of::<CMaterial>() as vk::DeviceSize,
                memory_type: MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            })
        ).unwrap();

        let handle = MaterialInstance {
            color_factors: glam::Vec4::ZERO,
            metal_rough_factors: glam::Vec4::ZERO,
            color_image: resources.color_image,
            color_image_sampler: resources.color_sampler,
            metal_rough_image: resources.metal_rough_image,
            metal_rough_image_sampler: resources.metal_sampler,
            data_buffer,
            gpu_rt: gpu_rt.clone(),
        };
        // upload
        gpu_rt.with_buffer_mut(&handle.data_buffer, |buffer| {
            let c_material = handle.to_c_material();
            buffer.upload(immediate, allocator, &[c_material]).unwrap();
        }).unwrap();
        handle
    }
}

impl MaterialInstance {
    pub fn to_c_material(&self) -> CMaterial {
        CMaterial {
            color_factors: self.color_factors,
            metal_rough_factors: self.metal_rough_factors,
            color_image: self.color_image.image.id() as u32,
            color_sampler: self.color_image_sampler.id() as u32,
            metal_image: self.metal_rough_image.image.id() as u32,
            metal_sampler: self.metal_rough_image_sampler.id() as u32,
        }
    }
}

#[repr(C)]
pub struct CMaterial {
    pub color_factors: glam::Vec4,
    pub metal_rough_factors: glam::Vec4,

    pub color_image: u32,
    pub color_sampler: u32,

    pub metal_image: u32,
    pub metal_sampler: u32,
}

pub struct RenderObject {
    index_count: u32,
    first_index: u32,

    material: Arc<MaterialInstance>,

    transform: glam::Mat4,
    vertex_handle: Handle<resource::Buffer<GPUAllocatorImpl>>,
    gpu_rt: GPUResourceTable<GPUAllocatorImpl>
}

impl Drop for RenderObject {
    fn drop(&mut self) {
        self.gpu_rt.free_buffer(self.vertex_handle.clone()).unwrap()
    }
}