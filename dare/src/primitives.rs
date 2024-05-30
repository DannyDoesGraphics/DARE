use std::mem;
use std::sync::Arc;

use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::free_list_allocator::Handle;

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
			.new_buffer(resource::BufferCreateInfo::NewEmptyBuffer {
				device: immediate.get_device().clone(),
				allocator,
				size: mem::size_of_val(vertices) as vk::DeviceSize,
				memory_type: dagal::allocators::MemoryLocation::GpuOnly,
				usage_flags: vk::BufferUsageFlags::TRANSFER_DST
					| vk::BufferUsageFlags::STORAGE_BUFFER
					| vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
			})
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

pub struct MaterialInner {
	pub color_image: Handle<resource::Image<GPUAllocatorImpl>>,
	pub color_image_view: Handle<resource::ImageView>,
	pub color_image_sampler: Handle<resource::Sampler>,

	pub metal_rough_image: Handle<resource::Image<GPUAllocatorImpl>>,
	pub metal_rough_image_view: Handle<resource::ImageView>,
	pub metal_rough_image_sampler: Handle<resource::Sampler>,

	pub gpu_rt: GPUResourceTable<GPUAllocatorImpl>,
}

/// Any information regarding the material of an object
#[derive(Clone)]
pub struct Material {
	pub color_factors: glam::Vec4,
	pub metal_rough_factors: glam::Vec4,

	inner: Arc<MaterialInner>,
}

impl Material {
	pub fn to_c_material(&self) -> CMaterial {
		CMaterial {
			color_factors: self.color_factors,
			metal_rough_factors: self.metal_rough_factors,
			color_image: self.inner.color_image.id() as u32,
			color_sampler: self.inner.color_image_sampler.id() as u32,
			metal_image: self.inner.metal_rough_image.id() as u32,
			metal_sampler: self.inner.metal_rough_image_sampler.id() as u32,
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