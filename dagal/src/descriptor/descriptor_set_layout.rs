use ash::vk;
use ash::vk::Handle;
use tracing::trace;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;

#[derive(Debug, Clone)]
pub struct DescriptorSetLayout {
	handle: vk::DescriptorSetLayout,
	device: crate::device::LogicalDevice,
	name: Option<String>,
}

pub enum DescriptorSetLayoutCreateInfo {
	/// Create a descriptor set layout from vk
	FromVk {
		handle: vk::DescriptorSetLayout,
		device: crate::device::LogicalDevice,
		name: Option<String>,
	}
}

impl<'a> Resource<'a> for DescriptorSetLayout {
	type CreateInfo = DescriptorSetLayoutCreateInfo;
	type HandleType = vk::DescriptorSetLayout;

	fn new(create_info: Self::CreateInfo) -> anyhow::Result<Self> where Self: Sized {
		match create_info {
			DescriptorSetLayoutCreateInfo::FromVk { handle, device, name } => {
				Ok(Self {
					handle,
					device,
					name
				})
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
		self.name = Some(name.to_string());
		Ok(())
	}

	fn get_name(&self) -> Option<&str> {
		self.name.as_deref()
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
