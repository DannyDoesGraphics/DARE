use ash::vk;
use crate::resource::traits::Resource;
use crate::traits::Destructible;
use anyhow::Result;
use ash::vk::Handle;
use tracing::trace;

#[derive(Debug, Clone)]
pub struct Sampler {
	handle: vk::Sampler,
	device: crate::device::LogicalDevice,
	name: Option<String>,
}

impl Destructible for Sampler {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroying VkSampler {:p}", self.handle);
		unsafe {
			self.device.get_handle().destroy_sampler(self.handle, None);
		}
	}
}

pub enum SamplerCreateInfo<'a> {
	FromCreateInfo {
		device: crate::device::LogicalDevice,
		create_info: vk::SamplerCreateInfo<'a>,
		name: Option<String>,
	}
}

impl<'a> Resource<'a> for Sampler {
	type CreateInfo = SamplerCreateInfo<'a>;
	type HandleType = vk::Sampler;

	fn new(create_info: Self::CreateInfo) -> Result<Self> where Self: Sized {
		match create_info {
			SamplerCreateInfo::FromCreateInfo { device, create_info, name } => {
				let handle = unsafe {
					device.get_handle().create_sampler(&create_info, None)
				}?;
				#[cfg(feature = "log-lifetimes")]
				trace!("Creating VkSampler {:p}", handle);

				let mut handle = Self {
					handle,
					device,
					name,
				};
				if let Some(debug_utils) = handle.device.clone().get_debug_utils() {
					if let Some(name) = handle.name.clone() {
						handle.set_name(debug_utils, name.as_str())?;
					}
				}

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

	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
		crate::resource::traits::name_resource(
			debug_utils,
			self.handle.as_raw(),
			vk::ObjectType::SAMPLER,
			name,
		)?;
		self.name = Some(name.to_string());
		Ok(())
	}

	fn get_name(&self) -> Option<&str> {
		self.name.as_deref()
	}
}