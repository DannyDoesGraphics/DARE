use std::{mem, ptr};
use std::ffi::c_void;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::allocators::{Allocator, GPUAllocatorImpl, SlotMapMemoryAllocator};
use crate::descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding;
use crate::resource::{Buffer, ImageCreateInfo, TypedBuffer, TypedBufferCreateInfo};
use crate::resource::traits::Resource;
use crate::traits::Destructible;
use crate::util::free_list_allocator::Handle;
use crate::util::FreeList;


#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct GPUResourceTable<A: Allocator = GPUAllocatorImpl> {
	device: crate::device::LogicalDevice,
	pool: crate::descriptor::DescriptorPool,
	set_layout: crate::descriptor::DescriptorSetLayout,
	descriptor_set: vk::DescriptorSet,
	#[derivative(Debug="ignore")]
	address_buffer: Buffer<A>,

	// Storage for the underlying resources
	images: FreeList<crate::resource::Image<A>>,
	image_views: FreeList<crate::resource::ImageView>,
	buffers: FreeList<Buffer<A>>,
}

const MAX_IMAGE_RESOURCES: u32 = 65536;
const MAX_BUFFER_RESOURCES: u32 = 65536;
const MAX_SAMPLER_RESOURCES: u32 = 1024;

const BUFFER_BINDING_INDEX: u32 = 3;
const STORAGE_IMAGE_BINDING_INDEX: u32 = 2;
const SAMPLED_IMAGE_BINDING_INDEX: u32 = 1;
const SAMPLER_BINDING_INDEX: u32 = 0;

impl<A: Allocator> GPUResourceTable<A> {
	pub fn new(device: crate::device::LogicalDevice, allocator: &mut SlotMapMemoryAllocator<A>) -> Result<Self> {
		let pool_sizes = [
			crate::descriptor::PoolSize::default()
				.descriptor_type(vk::DescriptorType::SAMPLER)
				.descriptor_count(MAX_SAMPLER_RESOURCES),
			crate::descriptor::PoolSize::default()
				.descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
				.descriptor_count(MAX_IMAGE_RESOURCES),
			crate::descriptor::PoolSize::default()
				.descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
				.descriptor_count(MAX_IMAGE_RESOURCES),
			crate::descriptor::PoolSize::default()
				.descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
				.descriptor_count(1)
		];

		let pool = crate::descriptor::DescriptorPool::new_with_pool_sizes(device.clone(), vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND, 1, pool_sizes.as_slice())?;
		let set_layout = crate::descriptor::DescriptorSetLayoutBuilder::default()
			.add_raw_binding(&[
				DescriptorSetLayoutBinding::default()
					.binding(SAMPLER_BINDING_INDEX)
					.descriptor_count(MAX_SAMPLER_RESOURCES)
					.descriptor_type(vk::DescriptorType::SAMPLER)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING),
				DescriptorSetLayoutBinding::default()
					.binding(SAMPLED_IMAGE_BINDING_INDEX)
					.descriptor_count(MAX_IMAGE_RESOURCES)
					.descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING),
				DescriptorSetLayoutBinding::default()
					.binding(STORAGE_IMAGE_BINDING_INDEX)
					.descriptor_count(MAX_IMAGE_RESOURCES)
					.descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING),
				DescriptorSetLayoutBinding::default()
					.binding(BUFFER_BINDING_INDEX)
					.descriptor_count(1)
					.descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
					.stage_flags(vk::ShaderStageFlags::ALL)
					.flag(vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING),
			])
			.build(device.clone(), vk::ShaderStageFlags::ALL, ptr::null(), vk::DescriptorSetLayoutCreateFlags::empty() )?;
		let descriptor_set = pool.allocate(set_layout.handle())?;
		// create a descriptor write
		let bda_buffer: Buffer<A> = Buffer::new(
			crate::resource::buffer::BufferCreateInfo::NewEmptyBuffer {
				device: device.clone(),
				allocator,
				size: MAX_BUFFER_RESOURCES as u64,
				memory_type: crate::allocators::MemoryLocation::CpuToGpu,
				usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER,
			}
		)?;

		/// bind bda buffer
		unsafe {
			device.get_handle().update_descriptor_sets(&[
				vk::WriteDescriptorSet {
					s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
					p_next: ptr::null(),
					dst_set: descriptor_set,
					dst_binding: BUFFER_BINDING_INDEX,
					dst_array_element: 0,
					descriptor_count: 1,
					descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
					p_image_info: ptr::null(),
					p_buffer_info: &vk::DescriptorBufferInfo {
						buffer: bda_buffer.handle(),
						offset: 0,
						range: vk::WHOLE_SIZE,
					},
					p_texel_buffer_view: ptr::null(),
					_marker: Default::default(),
				}
			], &[]);
		};

		Ok(Self {
			device,
			pool,
			set_layout,
			descriptor_set,
			address_buffer: bda_buffer,
			images: FreeList::default(),
			image_views: FreeList::default(),
			buffers: FreeList::default(),
		})
	}

	/// Get the underlying [`VkDescriptorSet`](vk::DescriptorSet) of the GPU resource table for
	/// the BDA buffer
	pub fn get_descriptor_set(&self) -> vk::DescriptorSet {
		self.descriptor_set
	}

	/// Get the underlying [VkDevice](ash::Device)
	pub fn get_device(&self) -> &crate::device::LogicalDevice {
		&self.device
	}

	pub fn get_descriptor_layout(&self) -> vk::DescriptorSetLayout {
		self.set_layout.handle()
	}

	pub fn new_image(&mut self, image_ci: ImageCreateInfo<A>, sampler: vk::Sampler, mut image_view: vk::ImageViewCreateInfo, image_layout: vk::ImageLayout)
		-> Result<Handle<crate::resource::Image<A>>> {
		let flags: vk::ImageUsageFlags = match &image_ci {
			ImageCreateInfo::FromVkNotManaged { .. } => { unimplemented!() } /// We only manage managed images
			ImageCreateInfo::NewUnallocated { image_ci, .. } => { image_ci.usage }
			ImageCreateInfo::NewAllocated { image_ci, .. } => { image_ci.usage }
		};
		let image = crate::resource::Image::new(image_ci)?;
		let vk_image = image.handle();
		let handle = self.images.allocate(image)?;
		image_view.image = vk_image;
		let image_view = unsafe {
			self.device.get_handle().create_image_view(&image_view, None)
		}?;

		let mut write_infos: Vec<vk::WriteDescriptorSet> = Vec::new();
		if flags & vk::ImageUsageFlags::SAMPLED == vk::ImageUsageFlags::SAMPLED {
			write_infos.push(vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.descriptor_set,
				dst_binding: SAMPLED_IMAGE_BINDING_INDEX,
				dst_array_element: handle.id() as u32,
				descriptor_count: 1,
				descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
				p_image_info: &vk::DescriptorImageInfo {
					sampler,
					image_view,
					image_layout,
				},
				p_buffer_info: ptr::null(),
				p_texel_buffer_view: ptr::null(),
				_marker: Default::default(),
			});
		}
		if flags & vk::ImageUsageFlags::STORAGE == vk::ImageUsageFlags::STORAGE {
			write_infos.push(vk::WriteDescriptorSet {
				s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
				p_next: ptr::null(),
				dst_set: self.descriptor_set,
				dst_binding: STORAGE_IMAGE_BINDING_INDEX,
				dst_array_element: handle.id() as u32,
				descriptor_count: 1,
				descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
				p_image_info: &vk::DescriptorImageInfo {
					sampler,
					image_view,
					image_layout,
				},
				p_buffer_info: ptr::null(),
				p_texel_buffer_view: ptr::null(),
				_marker: Default::default(),
			});
		}
		unsafe {
			self.device.get_handle().update_descriptor_sets(&write_infos, &[]);
		}
		
		Ok(handle)
	}

	/// Create a new buffer and put it into the bindless buffer
	///
	/// We expect every buffer created to have a SHADER_DEVICE_ADDRESS flag enabled
	pub fn new_buffer(&mut self, buffer_ci: crate::resource::BufferCreateInfo<A>)
		-> Result<Handle<Buffer<A>>> {
		/// confirm that BDA is enabled
		match buffer_ci {
			crate::resource::BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } => {
				if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS != vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS {
					return Err(anyhow::Error::from(crate::DagalError::NoShaderDeviceAddress))
				}
			}
		}

		let buffer: Buffer<A> = Buffer::new(buffer_ci)?;
		let buffer_address = buffer.address();
		let handle = self.buffers.allocate(buffer)?;
		// expand into the slot
		unsafe {
			let target_ptr = self.address_buffer.mapped_ptr().unwrap().as_ptr().add( mem::size_of::<vk::DeviceAddress>() * handle.id() as usize);
			let data_ptr = &buffer_address as *const _ as *const c_void;
			ptr::copy_nonoverlapping(data_ptr, target_ptr, mem::size_of::<vk::DeviceAddress>());
		}
		Ok(handle)
	}

	pub fn free_buffer(&mut self, handle: Handle<Buffer<A>>) -> Result<()> {
		self.buffers.deallocate_destructible(handle)
	}

	pub fn new_typed_buffer<T: Sized>(&mut self, buffer_ci: crate::resource::BufferCreateInfo<A>)
	-> Result<Handle<TypedBuffer<T, A>>> {
		let handle = self.new_buffer(buffer_ci)?;
		let handle: Handle<TypedBuffer<T, A>> = Handle::new(handle.id());
		Ok(handle)
	}

	pub fn free_typed_buffer<T: Sized>(&mut self, handle: Handle<TypedBuffer<T>>) -> Result<()> {
		let handle = Handle::new(handle.id());
		self.buffers.deallocate_destructible(handle)
	}

	/// Get buffer
	pub fn get_buffer(&self, handle: &Handle<Buffer<A>>) -> Result<Buffer<A>> {
		self.buffers.get(handle)
	}

	/// Get typed buffer
	pub fn get_typed_buffer<T: Sized>(&self, handle: &Handle<TypedBuffer<T, A>>) -> Result<TypedBuffer<T, A>> {
		let buffer = unsafe { self.buffers.untyped_get(handle)? };
		let buffer: Result<TypedBuffer<T, A>> = TypedBuffer::new(TypedBufferCreateInfo::FromDagalBuffer {
			handle: buffer
		});
		buffer
	}

	/// Get even more images
	pub fn get_image(&self, handle: &Handle<crate::resource::Image<A>>) -> Result<crate::resource::Image<A>> {
		self.images.get(handle)
	}
}

impl Destructible for GPUResourceTable {
	fn destroy(&mut self) {
		self.pool.destroy();
		self.set_layout.destroy();
		self.address_buffer.destroy();
	}
}

#[cfg(feature = "raii")]
impl Drop for GPUResourceTable {
	fn drop(&mut self) {
		self.destroy();
	}
}