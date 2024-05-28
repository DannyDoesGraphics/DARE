use dagal::allocators::{Allocator, GPUAllocatorImpl};

#[derive(Debug)]
pub struct Mesh<A: Allocator = GPUAllocatorImpl> {
	name: Option<String>,
	position: glam::Vec3,
	scale: glam::Vec3,
	vertex_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<A>>,
	normal_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<A>>,
	tangent_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<A>>,
	index_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<A>>,
	uv_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<A>>,
}

#[repr(C)]
#[derive(Debug)]
pub struct CMesh {
	position: glam::Vec3,
	scale: glam::Vec3,
	vertex_buffer: u32,
	normal_buffer: u32,
	tangent_buffer: u32,
	index_buffer: u32,
	uv_buffer: u32,
}

impl From<Mesh> for CMesh {
	fn from(value: Mesh) -> Self {
		Self {
			position: value.position,
			scale: value.scale,
			vertex_buffer: 0,
			normal_buffer: 0,
			tangent_buffer: 0,
			index_buffer: 0,
			uv_buffer: 0,
		}
	}
}