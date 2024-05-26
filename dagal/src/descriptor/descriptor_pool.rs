use std::ptr;

use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use tracing::trace;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;

/// Allocates descriptor set layouts
#[derive(Debug, Clone)]
pub struct DescriptorPool {
	handle: vk::DescriptorPool,
	device: crate::device::LogicalDevice,
	name: Option<String>,
}

/// If you want to allocate descriptors based on a ratio
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug, Default)]
pub struct PoolSizeRatio {
	pub descriptor_type: vk::DescriptorType,
	pub ratio: f32,
}

impl PoolSizeRatio {
	pub fn descriptor_type(mut self, descriptor_type: vk::DescriptorType) -> Self {
		self.descriptor_type = descriptor_type;
		self
	}
	pub fn ratio(mut self, ratio: f32) -> Self {
		self.ratio = ratio;
		self
	}
}

/// Create information for a [`DescriptorPool`].
///
/// # FromPoolSizeRatios
/// All ratios inputted in  `ratios` vector will be scaled by `count`
pub enum DescriptorPoolCreateInfo {
	FromVk {
		handle: vk::DescriptorPool,

		device: crate::device::LogicalDevice,
		name: Option<String>,
	},

	/// Allocate a pool from descriptor pool sizes
	FromPoolSizes {
		sizes: Vec<vk::DescriptorPoolSize>,
		flags: vk::DescriptorPoolCreateFlags,
		max_sets: u32,

		device: crate::device::LogicalDevice,
		name: Option<String>,
	},

	FromPoolSizeRatios {
		ratios: Vec<PoolSizeRatio>,
		/// Scale the ratios by
		count: u32,
		flags: vk::DescriptorPoolCreateFlags,
		max_sets: u32,

		device: crate::device::LogicalDevice,
		name: Option<String>,
	}
}

impl<'a> Resource<'a> for DescriptorPool {
	type CreateInfo = DescriptorPoolCreateInfo;
	type HandleType = vk::DescriptorPool;

	fn new(create_info: Self::CreateInfo) -> Result<Self> where Self: Sized {
		match create_info {
			DescriptorPoolCreateInfo::FromVk { handle, device, name } => {
				let mut handle = Self {
					handle,
					device,
					name
				};
				crate::resource::traits::update_name(&mut handle);
				Ok(handle)
			}
			DescriptorPoolCreateInfo::FromPoolSizes { sizes, flags, max_sets, device, name } => {
				let pool_ci = vk::DescriptorPoolCreateInfo {
					s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
					p_next: ptr::null(),
					flags,
					max_sets,
					pool_size_count: sizes.len() as u32,
					p_pool_sizes: sizes.as_ptr(),
					_marker: Default::default(),
				};

				let handle = unsafe { device.get_handle().create_descriptor_pool(&pool_ci, None)? };
				#[cfg(feature = "log-lifetimes")]
				trace!("Creating VkDescriptorPool {:p}", handle);
				let mut handle = Self { handle, device, name };
				crate::resource::traits::update_name(&mut handle);
				Ok(handle)
			}
			DescriptorPoolCreateInfo::FromPoolSizeRatios { ratios, count, flags, max_sets, device, name } => {
				let sizes: Vec<vk::DescriptorPoolSize> = ratios.into_iter().map(|ratio| {
					vk::DescriptorPoolSize {
						ty: ratio.descriptor_type,
						descriptor_count: (ratio.ratio * count as f32).round() as u32,
					}
				}).collect();
				Self::new(DescriptorPoolCreateInfo::FromPoolSizes {
					sizes,
					flags,
					max_sets,
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

impl Nameable for DescriptorPool {
	const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::DESCRIPTOR_POOL;
	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> anyhow::Result<()> {
		crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
		self.name = Some(name.to_string());
		Ok(())
	}

	fn get_name(&self) -> Option<&str> {
		self.name.as_deref()
	}
}

impl DescriptorPool {
	/// Resets a descriptor pool
	pub fn reset(&mut self, flags: vk::DescriptorPoolResetFlags) -> Result<()> {
		unsafe {
			self.device
			    .get_handle()
			    .reset_descriptor_pool(self.handle, flags)?
		};
		Ok(())
	}
}

impl Destructible for DescriptorPool {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroyed VkDescriptorPool {:p}", self.handle);

		unsafe {
			self.device
			    .get_handle()
			    .destroy_descriptor_pool(self.handle, None);
		}
	}
}

#[cfg(feature = "raii")]
impl Drop for DescriptorPool {
	fn drop(&mut self) {
		self.destroy();
	}
}
