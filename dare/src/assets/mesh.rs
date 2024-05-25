
#[derive(Debug)]
pub struct Mesh {
	name: Option<String>,
	position: glam::Vec3,
	scale: glam::Vec3,
	vertex_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<u8>>,
	normal_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<u8>>,
	tangent_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<u8>>,
	index_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<u8>>,
	uv_buffer: dagal::util::slot_map::Slot<dagal::resource::Buffer<u8>>,
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