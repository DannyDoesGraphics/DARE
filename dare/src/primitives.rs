use std::mem;

use dagal::allocators::{Allocator, GPUAllocatorImpl};
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
		let mut index_buffer = resource::Buffer::<GPUAllocatorImpl>::new(
			resource::BufferCreateInfo::NewEmptyBuffer {
				device: immediate.get_device().clone(),
				allocator,
				size: mem::size_of_val(indices) as vk::DeviceSize,
				memory_type: dagal::allocators::MemoryLocation::GpuOnly,
				usage_flags: vk::BufferUsageFlags::TRANSFER_DST
					| vk::BufferUsageFlags::INDEX_BUFFER,
			},
		)
			.unwrap();
		let vertex_buffer_handle = gpu_resource_table.new_buffer(
			resource::BufferCreateInfo::NewEmptyBuffer {
				device: immediate.get_device().clone(),
				allocator,
				size: mem::size_of_val(vertices) as vk::DeviceSize,
				memory_type: dagal::allocators::MemoryLocation::GpuOnly,
				usage_flags: vk::BufferUsageFlags::TRANSFER_DST
					| vk::BufferUsageFlags::STORAGE_BUFFER
					| vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
			}
		).unwrap();
		index_buffer.upload(immediate, allocator, indices).unwrap(); // fuck it lol
		gpu_resource_table.with_buffer_mut(&vertex_buffer_handle, |buffer| {
			buffer.upload::<Vertex>(immediate, allocator, vertices).unwrap();
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
		}).unwrap();

		if let Some(debug_utils) = immediate.get_device().get_debug_utils() {
			if let Some(name) = name {
				let index_name = {
					let mut n = name.clone();
					n.push_str(" index");
					n
				};
				index_buffer.set_name(debug_utils, index_name.as_str()).unwrap();
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

#[derive(Debug, Clone)]
pub enum MaterialPass {}

#[derive(Debug)]
pub struct MaterialInstance {
	material_pipeline: dagal::pipelines::GraphicsPipeline,
	material_set: dagal::descriptor::DescriptorSet,
	pass_type: MaterialPass,
}

#[derive(Debug)]
pub struct RenderObject<A: Allocator = GPUAllocatorImpl> {
	index_count: u32,
	first_index: u32,
	index_buffer: resource::Buffer<A>,
}

pub struct DrawContext {}

pub trait Renderable {
	fn draw(&self, matrix: &glam::Mat4, draw_context: &DrawContext);
}