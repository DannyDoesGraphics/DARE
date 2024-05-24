use std::ptr;
use std::sync::Arc;
use ash::vk;
use derivative::Derivative;

#[derive(Copy, Clone)]
pub union DescriptorInfo {
	buffer: vk::DescriptorBufferInfo,
	image: vk::DescriptorImageInfo,
}

impl Default for DescriptorInfo {
	fn default() -> Self {
		Self {
			buffer: Default::default()
		}
	}
}

#[derive(Clone, Default, Derivative)]
#[derivative(Debug)]
pub struct DescriptorWriteInfo {
	slot: u32,
	binding: u32,
	ty: vk::DescriptorType,
	#[derivative(Debug="ignore")]
	descriptors: Vec<DescriptorInfo>,
}

#[derive(Debug, Clone)]
pub struct DescriptorSet {
	handle: vk::DescriptorSet,
	device: crate::device::LogicalDevice,
}



impl DescriptorSet {
	pub fn write(&self, writes: &[DescriptorWriteInfo]) {
		/*
		let mut descriptor_writes: Vec<vk::WriteDescriptorSet> = Vec::with_capacity(writes.len());
		let mut descriptor_buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::with_capacity(writes.len());
		let mut descriptor_image_infos: Vec<vk::DescriptorImageInfo> =  Vec::with_capacity(writes.len());

		for write in writes.iter() {
			descriptor_writes.push(vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.handle,
				dst_binding: write.binding,
				dst_array_element: write.slot,
				descriptor_count: write.descriptors.len() as u32,
				descriptor_type: Default::default(),
				p_image_info: (),
				p_buffer_info: (),
				p_texel_buffer_view: (),
				_marker: Default::default(),
			});
		}

		unsafe {
			self.device.get_handle()
				.update_descriptor_sets(descriptor_writes.as_slice(), &[]);
		}
		 */
	}
}