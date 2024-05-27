use std::marker::PhantomData;
use std::mem;

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use crate::device::LogicalDevice;
use crate::resource::{Buffer, BufferCreateInfo};
use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;

/// Create a typed buffer
#[derive(Debug)]
pub struct TypedBuffer<T: Sized, A: Allocator = GPUAllocatorImpl> {
	handle: Buffer<A>,
	_marker: PhantomData<T>,
}

impl<T: Sized, A: Allocator> Destructible for TypedBuffer<T, A> {
	fn destroy(&mut self) {
		self.handle.destroy();
	}
}

impl<T: Sized, A: Allocator> Clone for TypedBuffer<T, A> {
	fn clone(&self) -> Self {
		Self {
			handle: self.handle.clone(),
			_marker: Default::default(),
		}
	}
}

pub enum TypedBufferCreateInfo<'a, A: Allocator> {
	FromDagalBufferCI {
		handle: BufferCreateInfo<'a, A>,
	},
	FromDagalBuffer {
		handle: Buffer<A>,
	}
}

impl<'a, T: Sized, A: Allocator + 'a> Resource<'a> for TypedBuffer<T, A> {
	type CreateInfo = TypedBufferCreateInfo<'a, A>;
	type HandleType = vk::Buffer;

	/// All size info is assumed to by scaled by the size of the type in the buffer
	fn new(create_info: Self::CreateInfo) -> Result<Self> where Self: Sized {
		match create_info {
			TypedBufferCreateInfo::FromDagalBufferCI { mut handle } => {
				match &mut handle {
					BufferCreateInfo::NewEmptyBuffer { size, .. } => {
						*size *= mem::size_of::<T>() as vk::DeviceSize;
					}
				}
				let handle = Buffer::new(handle)?;
				Ok(Self {
					handle,
					_marker: Default::default(),
				})
			}
			TypedBufferCreateInfo::FromDagalBuffer { handle } => {
				Ok(Self {
					handle,
					_marker: Default::default()
				})
			}
		}
	}

	fn get_handle(&self) -> &Self::HandleType {
		self.handle.get_handle()
	}

	fn handle(&self) -> Self::HandleType {
		self.handle.handle()
	}

	fn get_device(&self) -> &LogicalDevice {
		self.handle.get_device()
	}
}

impl<T: Sized> Nameable for TypedBuffer<T> {
	const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::BUFFER;
	fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
		self.handle.set_name(debug_utils, name)
	}

	fn get_name(&self) -> Option<&str> {
		self.handle.get_name()
	}
}

impl<T: Sized, A: Allocator> TypedBuffer<T, A> {
	/// Upload into the typed buffer using the type exclusively
	pub fn upload(&mut self,
	              immediate: &mut crate::util::ImmediateSubmit,
	              allocator: &mut ArcAllocator<A>,
	              content: &[T]) -> Result<()> {
		self.handle.upload(immediate, allocator, content)?;
		Ok(())
	}

	/// Get underlying untyped buffer
	pub fn get_untyped_buffer(&self) -> &Buffer<A> {
		&self.handle
	}
}