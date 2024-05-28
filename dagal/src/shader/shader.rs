use std::io::Read;

use anyhow::Result;
use ash::vk;
use tracing::trace;

use crate::traits::Destructible;

#[derive(Debug)]
pub struct Shader {
	handle: vk::ShaderModule,
	device: crate::device::LogicalDevice,
}

impl Shader {
	/// Creates a shader from a file
	pub fn from_file(
		device: crate::device::LogicalDevice,
		path: std::path::PathBuf,
	) -> Result<Self> {
		let mut buf: Vec<u8> = Vec::new();
		let mut file = std::fs::File::open(path)?;
		file.read_to_end(&mut buf)?;
		let content = ash::util::read_spv(&mut std::io::Cursor::new(buf))?;

		Self::from_slice(device, content.as_ref())
	}

	/// Create a shader from u32 slice
	pub fn from_slice(device: crate::device::LogicalDevice, content: &[u32]) -> Result<Self> {
		let shader_ci = vk::ShaderModuleCreateInfo::default().code(content);
		let handle = unsafe { device.get_handle().create_shader_module(&shader_ci, None)? };

		#[cfg(feature = "log-lifetimes")]
		trace!("Creating VkShaderModule {:p}", handle);

		Ok(Self { handle, device })
	}

	pub fn handle(&self) -> vk::ShaderModule {
		self.handle
	}
}

impl Destructible for Shader {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroying VkShaderModule {:p}", self.handle);

		unsafe {
			self.device
			    .get_handle()
			    .destroy_shader_module(self.handle, None);
		}
	}
}

#[cfg(feature = "raii")]
impl Drop for Shader {
	fn drop(&mut self) {
		self.destroy();
	}
}
