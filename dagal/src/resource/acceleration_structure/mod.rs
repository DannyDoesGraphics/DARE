
use ash::ext::debug_utils::Device;
use ash::vk;
use ash::vk::Handle;
use crate::resource::traits::Resource;
use crate::traits::Destructible;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct AccelerationStructure {
	handle: vk::AccelerationStructureKHR,
	ty: vk::AccelerationStructureTypeKHR,
	device: crate::device::LogicalDevice,
	name: Option<String>,
}

impl Destructible for AccelerationStructure {
	fn destroy(&mut self) {
		unsafe {
			self.device.get_acceleration_structure().unwrap().destroy_acceleration_structure(self.handle, None);
		}
	}
}

#[derive(Debug, Clone)]
pub enum AccelerationStructureCreateInfo {
	FromVk {
		handle: vk::AccelerationStructureKHR,
		ty: vk::AccelerationStructureTypeKHR,
		device: crate::device::LogicalDevice,
		name: Option<String>,
	}
}

impl Resource<'_> for AccelerationStructure {
	type CreateInfo = AccelerationStructureCreateInfo;
	type HandleType = vk::AccelerationStructureKHR;

	fn new(create_info: Self::CreateInfo) -> Result<Self> {
		match create_info {
			AccelerationStructureCreateInfo::FromVk { handle, ty, device, name } => {
				let mut handle = Self {
					handle,
					ty,
					device,
					name,
				};
				if let Some(debug_utils) = handle.device.clone().get_debug_utils() {
					if let Some(name) = handle.get_name() {
						let name = name.to_string();
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

	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> anyhow::Result<()> {
		crate::resource::traits::name_resource(
			debug_utils,
			self.handle.as_raw(),
			vk::ObjectType::ACCELERATION_STRUCTURE_KHR,
			name,
		)?;
		self.name = Some(name.to_string());
		Ok(())
	}

	fn get_name(&self) -> Option<&str> {
		self.name.as_deref()
	}
}
