use ash::vk;
use ash::vk::Handle;
use tracing::trace;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;

#[derive(Debug)]
pub struct DescriptorSetLayout {
	handle: vk::DescriptorSetLayout,
	device: crate::device::LogicalDevice,
}

pub enum DescriptorSetLayoutCreateInfo<'a> {
	/// Create a descriptor set layout from vk
	FromVk {
		handle: vk::DescriptorSetLayout,
		device: crate::device::LogicalDevice,
		name: Option<&'a str>,
	}
}

impl<'a> Resource<'a> for DescriptorSetLayout {
	type CreateInfo = DescriptorSetLayoutCreateInfo<'a>;
	type HandleType = vk::DescriptorSetLayout;

	fn new(create_info: Self::CreateInfo) -> anyhow::Result<Self> where Self: Sized {
		match create_info {
			DescriptorSetLayoutCreateInfo::FromVk { handle, device, name } => {
				let mut handle = Self {
					handle,
					device,
				};
				crate::resource::traits::update_name(&mut handle, name).unwrap_or(Ok(()))?;
				Ok(handle)
			}
		}
	}


	fn get_handle(&self) -> &Self::HandleType {
		&self.handle
	}

	fn handle(&self) -> Self::HandleType {
		self.handle
	}

	fn get_device(&self) -> &crate::device::LogicalDevice {
		&self.device
	}
}

impl Nameable for DescriptorSetLayout {
	const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::DESCRIPTOR_SET_LAYOUT;
	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> anyhow::Result<()> {
		crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
		Ok(())
	}
}

impl Destructible for DescriptorSetLayout {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroying VkDescriptorLayout {:p}", self.handle);
		unsafe {
			self.device
			    .get_handle()
			    .destroy_descriptor_set_layout(self.handle, None);
		}
	}
}

#[cfg(feature = "raii")]
impl Drop for DescriptorSetLayout {
	fn drop(&mut self) {
		self.destroy();
	}
}
