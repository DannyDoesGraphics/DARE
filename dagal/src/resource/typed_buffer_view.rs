use std::marker::PhantomData;

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use crate::device::LogicalDevice;
use crate::resource::Buffer;
use crate::resource::traits::{Nameable, Resource};

/// Create a typed buffer view into a [`Buffer`]
#[derive(Debug)]
pub struct TypedBufferView<'a, T: Sized, A: Allocator = GPUAllocatorImpl> {
	buffer: &'a mut Buffer<A>,
	_marker: PhantomData<T>,
}

pub enum TypedBufferCreateInfo<'a, A: Allocator> {
	FromDagalBuffer {
		buffer: &'a mut Buffer<A>,
	}
}

impl<'a, T: Sized, A: Allocator + 'a> Resource<'a> for TypedBufferView<'a, T, A> {
	type CreateInfo = TypedBufferCreateInfo<'a, A>;
	type HandleType = vk::Buffer;

	/// All size info is assumed to by scaled by the size of the type in the buffer
	fn new(create_info: Self::CreateInfo) -> Result<Self> where Self: Sized {
		match create_info {
			TypedBufferCreateInfo::FromDagalBuffer { buffer: handle } => {
				Ok(Self {
					buffer: handle,
					_marker: Default::default()
				})
			}
		}
	}

	fn get_handle(&self) -> &Self::HandleType {
		self.buffer.get_handle()
	}

	fn handle(&self) -> Self::HandleType {
		self.buffer.handle()
	}

	fn get_device(&self) -> &LogicalDevice {
		self.buffer.get_device()
	}
}

impl<'a, T: Sized, A: Allocator> Nameable for TypedBufferView<'a, T, A> {
	const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::BUFFER;
	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
		self.buffer.set_name(debug_utils, name)
	}
}

impl<'a, T: Sized, A: Allocator> TypedBufferView<'a, T, A> {
	/// Upload into the typed buffer using the type exclusively
	pub fn upload(&mut self,
	              immediate: &mut crate::util::ImmediateSubmit,
	              allocator: &mut ArcAllocator<A>,
	              content: &[T]) -> Result<()> {
		self.buffer.upload(immediate, allocator, content)?;
		Ok(())
	}

	/// Get reference to underlying untyped buffer
	pub fn get_untyped_buffer(&self) -> &Buffer<A> {
		&self.buffer
	}
}