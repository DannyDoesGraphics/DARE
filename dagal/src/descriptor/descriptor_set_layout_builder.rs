use std::ffi::c_void;
use std::ptr;

use anyhow::Result;
use ash::vk;

use crate::resource::traits::Resource;

#[derive(Clone, Debug, Default)]
pub struct DescriptorSetLayoutBuilder<'a> {
	bindings: Vec<DescriptorSetLayoutBinding<'a>>,
}

#[derive(Clone, Debug, Default)]
pub struct DescriptorSetLayoutBinding<'a> {
	handle: vk::DescriptorSetLayoutBinding<'a>,
	flags: vk::DescriptorBindingFlags,
}

impl<'a> DescriptorSetLayoutBinding<'a> {
	pub fn flag(mut self, flag: vk::DescriptorBindingFlags) -> Self {
		self.flags = flag;
		self
	}

	pub fn binding(mut self, binding: u32) -> Self {
		self.handle.binding = binding;
		self
	}

	pub fn descriptor_type(mut self, ty: vk::DescriptorType) -> Self {
		self.handle.descriptor_type = ty;
		self
	}

	pub fn descriptor_count(mut self, count: u32) -> Self {
		self.handle.descriptor_count = count;
		self
	}

	pub fn stage_flags(mut self, stage_flags: vk::ShaderStageFlags) -> Self {
		self.handle.stage_flags = stage_flags;
		self
	}
}


impl<'a> DescriptorSetLayoutBuilder<'a> {
	/// Adds a binding to be built
	pub fn add_binding(mut self, binding: u32, ty: vk::DescriptorType) -> Self {
		self.bindings.push(
			DescriptorSetLayoutBinding::default()
				.binding(binding)
				.descriptor_type(ty)
				.descriptor_count(1)
				.stage_flags(vk::ShaderStageFlags::ALL)
		);
		self
	}

	pub fn add_raw_binding(mut self, bindings: &[DescriptorSetLayoutBinding<'a>]) -> Self {
		self.bindings.extend_from_slice(bindings);
		self
	}

	/// Clear of all bindings
	pub fn clear(&mut self) {
		self.bindings.clear();
	}

	/// Builds the descriptor layout
	///
	/// # Examples
	/// ```
	/// use std::ptr;
	/// use ash::vk;
	/// use dagal::allocators::GPUAllocatorImpl;
	/// use dagal::resource::traits::Resource;
	/// use dagal::util::tests::TestSettings;
	/// use dagal::gpu_allocator;
	/// let (instance, physical_device, device, queue, mut deletion_stack) = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
	/// let allocator = GPUAllocatorImpl::new(gpu_allocator::vulkan::AllocatorCreateDesc {
	///     instance: instance.get_instance().clone(),
	///     device: device.get_handle().clone(),
	///     physical_device: physical_device.handle().clone(),
	///     debug_settings: gpu_allocator::AllocatorDebugSettings {
	///         log_memory_information: false,
	///             log_leaks_on_shutdown: true,
	///             store_stack_traces: false,
	///             log_allocations: false,
	///             log_frees: false,
	///             log_stack_traces: false,
	///         },
	///         buffer_device_address: false,
	///         allocation_sizes: Default::default(),
	///  }).unwrap();
	/// let descriptor_set_layout = dagal::descriptor::DescriptorSetLayoutBuilder::default()
	/// .add_binding(0, vk::DescriptorType::SAMPLER)
	/// .build(device.clone(), ptr::null(), vk::DescriptorSetLayoutCreateFlags::empty(), None).unwrap();
	/// deletion_stack.push_resource(&descriptor_set_layout);
	/// deletion_stack.flush();
	/// ```
	pub fn build(
		self,
		device: crate::device::LogicalDevice,
		p_next: *const c_void,
		create_flags: vk::DescriptorSetLayoutCreateFlags,
		name: Option<String>,
	) -> Result<crate::descriptor::DescriptorSetLayout> {
		let raw_bindings: Vec<vk::DescriptorSetLayoutBinding> = self.bindings.iter().map(|binding| {
			binding.handle
		}).collect();
		let flags: Vec<vk::DescriptorBindingFlags> = self.bindings.iter().map(|binding| {
			binding.flags
		}).collect();
		let flags_enabled = flags.iter().any(|flag| {
			*flag != vk::DescriptorBindingFlags::default()
		});

		let binding_flags = vk::DescriptorSetLayoutBindingFlagsCreateInfo {
			s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_BINDING_FLAGS_CREATE_INFO,
			p_next,
			binding_count: flags.len() as u32,
			p_binding_flags: flags.as_ptr(),
			_marker: Default::default(),
		};

		let descriptor_set_layout_ci = vk::DescriptorSetLayoutCreateInfo {
			s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
			p_next: if !flags_enabled { ptr::null() } else { &binding_flags as *const _ as *const c_void },
			flags: create_flags,
			binding_count: raw_bindings.len() as u32,
			p_bindings: raw_bindings.as_ptr(),
			_marker: Default::default(),
		};
		let handle = unsafe {
			device
				.get_handle()
				.create_descriptor_set_layout(&descriptor_set_layout_ci, None)?
		};
		crate::descriptor::DescriptorSetLayout::new(crate::descriptor::DescriptorSetLayoutCreateInfo::FromVk {
			handle,
			device,
			name,
		})
	}
}
