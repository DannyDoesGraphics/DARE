use std::{mem, ptr};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use crate::DagalError::PoisonError;
use crate::descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding;
use crate::resource;
use crate::resource::traits::Resource;
use crate::util::free_list_allocator::Handle;
use crate::util::FreeList;

#[derive(Debug)]
struct GPUResourceTableInner<A: Allocator = GPUAllocatorImpl> {
	pool: crate::descriptor::DescriptorPool,
	set_layout: crate::descriptor::DescriptorSetLayout,
	descriptor_set: crate::descriptor::DescriptorSet,
	address_buffer: resource::Buffer<A>,
}

#[derive(Debug, Clone)]
pub struct GPUResourceTable<A: Allocator = GPUAllocatorImpl> {
	inner: Arc<RwLock<GPUResourceTableInner<A>>>,

	// Storage for the underlying resources
	images: FreeList<resource::Image<A>>,
	image_views: FreeList<resource::ImageView>,
	buffers: FreeList<resource::Buffer<A>>,
	samplers: FreeList<resource::Sampler>,

	device: crate::device::LogicalDevice,
}

const MAX_IMAGE_RESOURCES: u32 = 65536;
const MAX_BUFFER_RESOURCES: u32 = 65536;
const MAX_SAMPLER_RESOURCES: u32 = 1024;

const BUFFER_BINDING_INDEX: u32 = 3;
const STORAGE_IMAGE_BINDING_INDEX: u32 = 2;
const SAMPLED_IMAGE_BINDING_INDEX: u32 = 1;
const SAMPLER_BINDING_INDEX: u32 = 0;

pub enum ResourceInput<'a, T: Resource<'a>> {
	Resource(T),
	ResourceCI(T::CreateInfo),
	ResourceHandle(Handle<T>),
}

impl<A: Allocator> GPUResourceTable<A> {
	pub fn new(
		device: crate::device::LogicalDevice,
		allocator: &mut ArcAllocator<A>,
	) -> Result<Self> {
		let pool_sizes = vec![
			vk::DescriptorPoolSize::default()
				.ty(vk::DescriptorType::SAMPLER)
				.descriptor_count(MAX_SAMPLER_RESOURCES),
			vk::DescriptorPoolSize::default()
				.ty(vk::DescriptorType::SAMPLED_IMAGE)
				.descriptor_count(MAX_IMAGE_RESOURCES),
			vk::DescriptorPoolSize::default()
				.ty(vk::DescriptorType::STORAGE_IMAGE)
				.descriptor_count(MAX_IMAGE_RESOURCES),
			vk::DescriptorPoolSize::default()
				.ty(vk::DescriptorType::STORAGE_BUFFER)
				.descriptor_count(1),
		];

		let pool = crate::descriptor::DescriptorPool::new(
			crate::descriptor::DescriptorPoolCreateInfo::FromPoolSizes {
				sizes: pool_sizes,
				flags: vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND,
				max_sets: 1,
				device: device.clone(),
				name: None,
			},
		)?;
		let set_layout = crate::descriptor::DescriptorSetLayoutBuilder::default()
			.add_raw_binding(&[
				DescriptorSetLayoutBinding::default()
					.binding(SAMPLER_BINDING_INDEX)
					.descriptor_count(MAX_SAMPLER_RESOURCES)
					.descriptor_type(vk::DescriptorType::SAMPLER)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(
						vk::DescriptorBindingFlags::PARTIALLY_BOUND
							| vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
					),
				DescriptorSetLayoutBinding::default()
					.binding(SAMPLED_IMAGE_BINDING_INDEX)
					.descriptor_count(MAX_IMAGE_RESOURCES)
					.descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(
						vk::DescriptorBindingFlags::PARTIALLY_BOUND
							| vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
					),
				DescriptorSetLayoutBinding::default()
					.binding(STORAGE_IMAGE_BINDING_INDEX)
					.descriptor_count(MAX_IMAGE_RESOURCES)
					.descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(
						vk::DescriptorBindingFlags::PARTIALLY_BOUND
							| vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
					),
				DescriptorSetLayoutBinding::default()
					.binding(BUFFER_BINDING_INDEX)
					.descriptor_count(1)
					.descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(
						vk::DescriptorBindingFlags::PARTIALLY_BOUND
							| vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
					),
			])
			.build(
				device.clone(),
				ptr::null(),
				vk::DescriptorSetLayoutCreateFlags::empty(),
				None,
			)?;
		let descriptor_set = crate::descriptor::DescriptorSet::new(
			crate::descriptor::DescriptorSetCreateInfo::NewSet {
				pool: &pool,
				layout: &set_layout,
				name: Some("GPU resource table descriptor set"),
			},
		)?;
		// create a descriptor write
		let bda_buffer: resource::Buffer<A> =
			resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
				device: device.clone(),
				allocator,
				size: ((MAX_BUFFER_RESOURCES as usize) * mem::size_of::<vk::DeviceSize>()) as u64,
				memory_type: crate::allocators::MemoryLocation::CpuToGpu,
				usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER,
			})?;
		descriptor_set.write(&[crate::descriptor::DescriptorWriteInfo::default()
			.ty(crate::descriptor::DescriptorType::StorageBuffer)
			.binding(BUFFER_BINDING_INDEX)
			.slot(0)
			.push_descriptor(crate::descriptor::DescriptorInfo::Buffer(
				vk::DescriptorBufferInfo {
					buffer: bda_buffer.handle(),
					offset: 0,
					range: vk::WHOLE_SIZE,
				},
			))]);

		Ok(Self {
			inner: Arc::new(RwLock::new(GPUResourceTableInner {
				pool,
				set_layout,
				descriptor_set,
				address_buffer: bda_buffer,
			})),
			images: FreeList::default(),
			image_views: FreeList::default(),
			buffers: FreeList::default(),
			samplers: FreeList::default(),
			device,
		})
	}

	/// Get the underlying [`VkDescriptorSet`](vk::DescriptorSet) of the GPU resource table for
	/// the BDA buffer
	pub fn with_descriptor_set<R, F: FnOnce(&crate::descriptor::DescriptorSet) -> R>(
		&self,
		f: F,
	) -> Result<R> {
		let descriptor_set = &self
			.inner
			.read()
			.map_err(|_| anyhow::Error::from(crate::DagalError::NoShaderDeviceAddress))?
			.descriptor_set;
		Ok(f(descriptor_set))
	}

	pub fn get_descriptor_set(&self) -> Result<vk::DescriptorSet> {
		Ok(self
			.inner
			.read()
			.map_err(|_| anyhow::Error::from(crate::DagalError::NoShaderDeviceAddress))?
			.descriptor_set
			.handle())
	}

	/// Get the underlying [VkDevice](ash::Device)
	pub fn get_device(&self) -> &crate::device::LogicalDevice {
		&self.device
	}

	pub fn get_descriptor_layout(&self) -> Result<vk::DescriptorSetLayout> {
		Ok(self
			.inner
			.read()
			.map_err(|_| anyhow::Error::from(crate::DagalError::NoShaderDeviceAddress))?
			.set_layout
			.handle())
	}

	/// Create a new image view
	pub fn new_image_view(
		&mut self,
		image_view_ci: ResourceInput<resource::ImageView>,
	) -> Result<Handle<resource::ImageView>> {
		match image_view_ci {
			ResourceInput::Resource(resource) => self.image_views.allocate(resource),
			ResourceInput::ResourceCI(ci) => {
				let resource = resource::ImageView::new(ci)?;
				self.image_views.allocate(resource)
			}
			ResourceInput::ResourceHandle(handle) => Ok(handle),
		}
	}

	pub fn free_image_view(&mut self, handle: Handle<resource::ImageView>) -> Result<()> {
		self.image_views.deallocate_destructible(handle)
	}

	pub fn with_image_view<R, F: FnOnce(&resource::ImageView) -> R>(
		&self,
		handle: &Handle<resource::ImageView>,
		f: F,
	) -> Result<R> {
		self.image_views.with_handle(handle, f)
	}

	/// Get a new sampler
	pub fn new_sampler(
		&mut self,
		sampler: ResourceInput<resource::Sampler>,
	) -> Result<Handle<resource::Sampler>> {
		match sampler {
			ResourceInput::ResourceHandle(handle) => {
				return Ok(handle);
			}
			_ => {}
		};

		let sampler_handle = match sampler {
			ResourceInput::Resource(resource) => self.samplers.allocate(resource),
			ResourceInput::ResourceCI(create_info) => {
				let resource = resource::Sampler::new(create_info)?;
				self.samplers.allocate(resource)
			}
			_ => unimplemented!(),
		}?;
		let sampler = self.samplers.get_resource_handle(&sampler_handle)?;
		let p_image_info = vk::DescriptorImageInfo {
			sampler,
			image_view: vk::ImageView::null(),
			image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
		};
		unsafe {
			self.with_descriptor_set(|descriptor_set| {
				self.device.get_handle().update_descriptor_sets(
					&[vk::WriteDescriptorSet {
						s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
						p_next: ptr::null(),
						dst_set: descriptor_set.handle(),
						dst_binding: SAMPLER_BINDING_INDEX,
						dst_array_element: sampler_handle.id() as u32,
						descriptor_count: 1,
						descriptor_type: vk::DescriptorType::SAMPLER,
						p_image_info: &p_image_info,
						p_buffer_info: ptr::null(),
						p_texel_buffer_view: ptr::null(),
						_marker: Default::default(),
					}],
					&[],
				);
			})?;
		}

		Ok(sampler_handle)
	}

	/// Get a sampler from is handle
	pub fn with_sampler<R, F: FnOnce(&resource::Sampler) -> R>(
		&self,
		sampler: &Handle<resource::Sampler>,
		f: F,
	) -> Result<R> {
		self.samplers.with_handle(sampler, f)
	}

	/// Free a list sampler from the gpu resource table
	pub fn free_sampler(&mut self, sampler: Handle<resource::Sampler>) -> Result<()> {
		self.samplers.deallocate_destructible(sampler)
	}

	pub fn new_image<'a>(
		&mut self,
		image_ci: ResourceInput<'a, resource::Image<A>>,
		image_view_ci: ResourceInput<'a, resource::ImageView>,
		image_layout: vk::ImageLayout,
	) -> Result<(Handle<resource::Image<A>>, Handle<resource::ImageView>)>
		where
			A: 'a,
	{
		let image_handle = match image_ci {
			ResourceInput::Resource(image) => self.images.allocate(image)?,
			ResourceInput::ResourceCI(image_ci) => {
				let image = resource::Image::new(image_ci)?;
				self.images.allocate(image)?
			}
			ResourceInput::ResourceHandle(handle) => handle,
		};
		let image_view_handle = self.new_image_view(image_view_ci)?;
		let image_view = self.image_views.get_resource_handle(&image_view_handle)?;

		let image_flags: vk::ImageUsageFlags = self
			.images
			.with_handle(&image_handle, |image| image.usage_flags())?;
		let mut write_infos: Vec<vk::WriteDescriptorSet> = Vec::new();
		if image_flags & vk::ImageUsageFlags::SAMPLED == vk::ImageUsageFlags::SAMPLED {
			write_infos.push(vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.get_descriptor_set()?,
				dst_binding: SAMPLED_IMAGE_BINDING_INDEX,
				dst_array_element: image_handle.id() as u32,
				descriptor_count: 1,
				descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
				p_image_info: &vk::DescriptorImageInfo {
					sampler: vk::Sampler::null(),
					image_view,
					image_layout,
				},
				p_buffer_info: ptr::null(),
				p_texel_buffer_view: ptr::null(),
				_marker: Default::default(),
			});
		}
		if image_flags & vk::ImageUsageFlags::STORAGE == vk::ImageUsageFlags::STORAGE {
			write_infos.push(vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.get_descriptor_set()?,
				dst_binding: STORAGE_IMAGE_BINDING_INDEX,
				dst_array_element: image_handle.id() as u32,
				descriptor_count: 1,
				descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
				p_image_info: &vk::DescriptorImageInfo {
					sampler: vk::Sampler::null(),
					image_view,
					image_layout,
				},
				p_buffer_info: ptr::null(),
				p_texel_buffer_view: ptr::null(),
				_marker: Default::default(),
			});
		}
		unsafe {
			self.device
			    .get_handle()
			    .update_descriptor_sets(write_infos.as_slice(), &[]);
		}

		Ok((image_handle, image_view_handle))
	}

	pub fn free_image(&mut self, handle: Handle<resource::Image<A>>) -> Result<()> {
		self.images.deallocate_destructible(handle)
	}

	/// Create a new buffer and put it into the bindless buffer
	///
	/// We expect every buffer created to have a SHADER_DEVICE_ADDRESS flag enabled
	pub fn new_buffer<'a>(
		&mut self,
		buffer_input: ResourceInput<'a, resource::Buffer<A>>,
	) -> Result<Handle<resource::Buffer<A>>>
		where
			A: 'a
	{
		let handle = match buffer_input {
			ResourceInput::Resource(buffer) => {
				let buffer_address = buffer.address();
				let handle = self.buffers.allocate(buffer)?;
				self.inner
				    .write()
				    .map_err(|_| anyhow::Error::from(PoisonError))?
					.address_buffer
					.write(
						(mem::size_of::<vk::DeviceMemory>() * handle.id() as usize) as vk::DeviceSize,
						&[buffer_address],
					)?;
				handle
			},
			ResourceInput::ResourceCI(buffer_ci) => {
				match buffer_ci {
					resource::BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } => {
						if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
							!= vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
						{
							return Err(anyhow::Error::from(
								crate::DagalError::NoShaderDeviceAddress,
							));
						}
					}
				}

				let buffer: resource::Buffer<A> = resource::Buffer::new(buffer_ci)?;
				self.new_buffer(ResourceInput::Resource(buffer))?
			}
			ResourceInput::ResourceHandle(handle) => handle,
		};

		Ok(handle)
	}

	pub fn free_buffer(&mut self, handle: Handle<resource::Buffer<A>>) -> Result<()> {
		self.buffers.deallocate_destructible(handle)
	}

	/// Get buffer
	pub fn with_buffer<R, F: FnOnce(&resource::Buffer<A>) -> R>(
		&self,
		handle: &Handle<resource::Buffer<A>>,
		f: F,
	) -> Result<R> {
		self.buffers.with_handle(handle, f)
	}

	pub fn with_buffer_mut<R, F: FnOnce(&mut resource::Buffer<A>) -> R>(
		&mut self,
		handle: &Handle<resource::Buffer<A>>,
		f: F,
	) -> Result<R> {
		self.buffers.with_handle_mut(handle, f)
	}

	/// Get typed buffer
	pub fn with_typed_buffer<T: Sized, R, F: FnOnce(resource::TypedBufferView<T, A>) -> R>(
		&mut self,
		handle: &Handle<resource::TypedBufferView<T, A>>,
		f: F,
	) -> Result<R> {
		unsafe {
			self.buffers.untyped_with_handle_mut(handle, move |buffer| {
				let typed_buffer = resource::TypedBufferView::new(
					resource::TypedBufferCreateInfo::FromDagalBuffer { buffer },
				)
					.unwrap();
				f(typed_buffer)
			})
		}
	}

	/// Get even more images
	pub fn with_image<R, F: FnOnce(&resource::Image<A>) -> R>(
		&self,
		handle: &Handle<resource::Image<A>>,
		f: F,
	) -> Result<R> {
		self.images.with_handle(handle, f)
	}
}
