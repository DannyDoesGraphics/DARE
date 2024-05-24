use std::ptr;
use ash::vk;
use derivative::Derivative;

#[derive(Copy, Clone, Debug)]
pub enum DescriptorInfo {
	Buffer(vk::DescriptorBufferInfo),
	Image(vk::DescriptorImageInfo),
}

impl Default for DescriptorInfo {
	fn default() -> Self {
		Self::Buffer(Default::default())
	}
}

/// https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkDescriptorType.html
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Eq, Ord, Hash)]
pub enum DescriptorType {
	Sampler = 0,
	CombinedImageSampler = 1,
	SampledImage = 2,
	StorageImage = 3,
	UniformTexelBuffer = 4,
	StorageTexelBuffer = 5,
	UniformBuffer = 6,
	StorageBuffer = 7,
	UniformBufferDynamic = 8,
	StorageBufferDynamic = 9,
	InputAttachment = 10,
}
impl Default for DescriptorType {
	fn default() -> Self {
		Self::Sampler
	}
}
impl DescriptorType {
	pub fn to_vk(&self) -> vk::DescriptorType {
		vk::DescriptorType::from_raw(*self as i32)
	}
}

#[derive(Clone, Default, Derivative)]
#[derivative(Debug)]
pub struct DescriptorWriteInfo {
	slot: u32,
	binding: Option<u32>,
	ty: DescriptorType,
	#[derivative(Debug="ignore")]
	descriptors: Vec<DescriptorInfo>,
}

#[derive(Debug, Clone)]
pub struct DescriptorSet {
	handle: vk::DescriptorSet,
	device: crate::device::LogicalDevice,
}



impl DescriptorSet {

	/// Submit writes to the current descriptor set
	pub fn write(&self, writes: &[DescriptorWriteInfo]) {
		let mut descriptor_writes: Vec<vk::WriteDescriptorSet> = Vec::with_capacity(writes.len());
		let mut descriptor_buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::with_capacity(writes.len());
		let mut descriptor_image_infos: Vec<vk::DescriptorImageInfo> =  Vec::with_capacity(writes.len());

		for write in writes.iter() {
			let dst_binding = write.binding.unwrap();
			let descriptor_write = vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.handle,
				dst_binding,
				dst_array_element: write.slot,
				descriptor_count: 0,
				descriptor_type: write.ty.to_vk(),
				p_image_info: ptr::null(),
				p_buffer_info: ptr::null(),
				p_texel_buffer_view: ptr::null(),
				_marker: Default::default(),
			};
			match write.ty {
				DescriptorType::Sampler | DescriptorType::CombinedImageSampler | DescriptorType::SampledImage | DescriptorType::StorageImage | DescriptorType::InputAttachment => {
					let mut descriptor_count: u32 = 0;
					let start: usize = descriptor_image_infos.len();
					for descriptor in write.descriptors.iter() {
						match descriptor {
							DescriptorInfo::Image(descriptor) => {
								descriptor_count += 1;
								descriptor_image_infos.push(*descriptor)
							},
							_ => {},
						}
					}

					let mut descriptor_write = descriptor_write.clone();
					descriptor_write.descriptor_count = descriptor_count;
					descriptor_write.p_image_info = descriptor_image_infos[start..].as_ptr();
					descriptor_writes.push(descriptor_write);
				},
				DescriptorType::UniformBuffer | DescriptorType::StorageBuffer | DescriptorType::UniformBufferDynamic
				| DescriptorType::StorageBufferDynamic => {
					let mut descriptor_count: u32 = 0;
					let start: usize = descriptor_buffer_infos.len();
					for descriptor in write.descriptors.iter() {
						match descriptor {
							DescriptorInfo::Buffer(descriptor) => {
								descriptor_count += 1;
								descriptor_buffer_infos.push(*descriptor)
							},
							_ => {},
						}
					}

					let mut descriptor_write = descriptor_write.clone();
					descriptor_write.descriptor_count = descriptor_count;
					descriptor_write.p_buffer_info = descriptor_buffer_infos[start..].as_ptr();
					descriptor_writes.push(descriptor_write);
				},
				_ => unimplemented!(),
			}
		}

		unsafe {
			self.device.get_handle()
				.update_descriptor_sets(descriptor_writes.as_slice(), &[]);
		}
	}
}