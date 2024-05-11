use std::sync::atomic::fence;
use ash::vk;
use tracing::trace;
use crate::traits::Destructible;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct PipelineLayout {
	handle: vk::PipelineLayout,
	device: crate::device::LogicalDevice
}

impl PipelineLayout {
	pub fn new(device: crate::device::LogicalDevice, layout_ci: &vk::PipelineLayoutCreateInfo) -> Result<Self> {
		let handle = unsafe {
			device.get_handle().create_pipeline_layout(layout_ci, None)
		}?;
		Ok(Self {
			handle,
			device,
		})
	}

	pub fn from_vk(handle: vk::PipelineLayout, device: crate::device::LogicalDevice) -> Self {
		#[cfg(feature = "log-lifetimes")]
		trace!("Building a VkPipelineLayout from Vk {:p}", handle);
		Self {
			handle,
			device,
		}
	}

	pub fn handle(&self) -> vk::PipelineLayout {
		self.handle
	}
}

impl Destructible for PipelineLayout {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroying VkPipelineLayout {:p}", self.handle);

		unsafe {
			self.device.get_handle().destroy_pipeline_layout(self.handle, None);
		}
	}
}