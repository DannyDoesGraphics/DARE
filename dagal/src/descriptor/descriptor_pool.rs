use std::ptr;
use ash::vk;
use crate::traits::Destructible;
use anyhow::Result;

/// Allocates descriptor set layouts
pub struct DescriptorPool {
	handle: vk::DescriptorPool,
	device: crate::device::LogicalDevice,
}

struct PoolSizeRatio {
	descriptor_type: vk::DescriptorType,
	ratio: f32,
}

impl DescriptorPool {
	pub fn new(device: crate::device::LogicalDevice, max_sets: u32, pool_ratios: &[PoolSizeRatio]) -> Result<Self> {
		let pool_sizes: Vec<vk::DescriptorPoolSize> = pool_ratios.iter().map(|pool_ratio| {
			vk::DescriptorPoolSize {
				ty: pool_ratio.descriptor_type,
				descriptor_count: (pool_ratio.ratio * max_sets as f32).ceil() as u32,
			}
		}).collect();
		
		let pool_ci = vk::DescriptorPoolCreateInfo {
			s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
			p_next: ptr::null(),
			flags: vk::DescriptorPoolCreateFlags::empty(),
			max_sets,
			pool_size_count: pool_sizes.len() as u32,
			p_pool_sizes: pool_sizes.as_ptr(),
			_marker: Default::default(),
		};
		
		let handle  = unsafe {
			device.get_handle().create_descriptor_pool(&pool_ci, None)?
		};
		Ok(Self {
			handle,
			device,
		})
	}
	
	/// Resets a descriptor pool and clears it entirely
	pub fn reset(&mut self, flags: vk::DescriptorPoolResetFlags) -> Result<()> {
		unsafe {
			self.device.get_handle().reset_descriptor_pool(self.handle, flags)?
		};
		Ok(())
	}
	
	pub fn allocate(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet> {
		let alloc_info = vk::DescriptorSetAllocateInfo {
			s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
			p_next: ptr::null(),
			descriptor_pool: self.handle,
			descriptor_set_count: 1,
			p_set_layouts: &layout,
			_marker: Default::default(),
		};
		let mut handle = unsafe {
			self.device.get_handle().allocate_descriptor_sets(&alloc_info)?
		};
		Ok(handle.pop().unwrap())
	}
}

impl Destructible for DescriptorPool {
	fn destroy(&mut self) {
		unsafe {
			self.device.get_handle().destroy_descriptor_pool(self.handle, None);
		}
	}
}

#[cfg(feature = "raii")]
impl Drop for DescriptorPool {
	fn drop(&mut self) {
		self.destroy();
	}
}