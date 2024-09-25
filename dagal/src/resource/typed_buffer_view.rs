use std::marker::PhantomData;

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocator, GPUAllocatorImpl};
use crate::device::LogicalDevice;
use crate::resource::traits::{Nameable, Resource};
use crate::resource::Buffer;
use crate::traits::AsRaw;

/// Create a typed buffer view into a [`Buffer`]
#[derive(Debug)]
pub struct TypedBufferView<'a, T: Sized, A: Allocator = GPUAllocatorImpl> {
    buffer: &'a mut Buffer<A>,
    _marker: PhantomData<T>,
}

pub enum TypedBufferCreateInfo<'a, A: Allocator> {
    FromDagalBuffer { buffer: &'a mut Buffer<A> },
}

impl<'a, T: Sized, A: Allocator + 'a> Resource<'a> for TypedBufferView<'a, T, A> {
    type CreateInfo = TypedBufferCreateInfo<'a, A>;

    /// All size info is assumed to by scaled by the size of the type in the buffer
    fn new(create_info: Self::CreateInfo) -> Result<Self>
    where
        Self: Sized,
    {
        match create_info {
            TypedBufferCreateInfo::FromDagalBuffer { buffer: handle } => Ok(Self {
                buffer: handle,
                _marker: Default::default(),
            }),
        }
    }

    fn get_device(&self) -> &LogicalDevice {
        self.buffer.get_device()
    }
}

impl<'a, T: Sized, A: Allocator> AsRaw for TypedBufferView<'a, T, A> {
    type RawType = vk::Buffer;

    unsafe fn as_raw(&self) -> &Self::RawType {
        self.buffer.as_raw()
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        self.buffer.as_raw_mut()
    }

    unsafe fn raw(self) -> Self::RawType {
        unimplemented!()
    }
}

impl<'a, T: Sized, A: Allocator> Nameable for TypedBufferView<'a, T, A> {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::BUFFER;
    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
        self.buffer.set_name(debug_utils, name)
    }
}

impl<'a, T: Sized, A: Allocator> TypedBufferView<'a, T, A> {
    /// Get reference to underlying untyped buffer
    pub fn get_untyped_buffer(&self) -> &Buffer<A> {
        self.buffer
    }
}
