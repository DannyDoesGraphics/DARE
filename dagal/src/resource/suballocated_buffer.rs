use std::os::raw::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Weak};

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocator, GPUAllocatorImpl};
use crate::DagalError;
use crate::resource::traits::Resource;
use crate::util::Slot;

/// A suballocation inside a [`SuballocatedBuffer`]
#[derive(Debug, Clone)]
pub struct Suballocation<A: Allocator = GPUAllocatorImpl> {
    buffer: Weak<super::Buffer<A>>,
    mapped_ptr: Option<NonNull<c_void>>,
    offset: vk::DeviceSize,
    length: vk::DeviceSize,
}

impl<A: Allocator> Suballocation<A> {
    /// Acquire a mapped pointer, if one exists with the proper offsets
    pub fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        // SAFETY: it is already checked prior this suballocation can even fit inside the buffer
        self.buffer
            .upgrade()?
            .mapped_ptr()
            .and_then(|ptr| unsafe { NonNull::new(ptr.as_ptr().add(self.offset as usize)) })
    }

    /// Copy data to a mapped pointer on the buffer, if one exists
    pub fn copy_to<T>(&mut self, data: &[T]) -> Result<()> {
        if std::mem::size_of_val(data) > self.length as usize {
            return Err(anyhow::Error::from(DagalError::InsufficientSpace));
        }
        self.mapped_ptr.map_or(
            Err(anyhow::Error::from(DagalError::NoMappedPointer)),
            |mut mapped_ptr| {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr() as *const _ as *const c_void,
                        mapped_ptr.as_mut() as *mut _ as *mut c_void,
                        std::mem::size_of_val(data),
                    );
                }
                Ok(())
            },
        )
    }

    /// Get the offset
    pub fn offset(&self) -> vk::DeviceSize {
        self.offset
    }

    /// Get the length
    pub fn length(&self) -> vk::DeviceSize {
        self.length
    }
}

/// A suballocated [`Buffer`](super::Buffer)
#[derive(Debug)]
pub struct SuballocatedBuffer<A: Allocator = GPUAllocatorImpl> {
    /// we don't actually ref count, rather we use it to hold weak refs to
    buffer: Arc<super::Buffer<A>>,
    suballocations: crate::util::SparseSlotMap<Suballocation<A>>,
}

impl<'a, A: Allocator + 'a> Resource<'a> for SuballocatedBuffer<A> {
    type CreateInfo = super::BufferCreateInfo<'a, A>;
    type HandleType = vk::Buffer;

    fn new(create_info: Self::CreateInfo) -> Result<Self>
           where
               Self: Sized,
    {
        let buffer: Arc<super::Buffer<A>> = Arc::new(super::Buffer::new(create_info)?);
        Ok(Self {
            buffer,
            suballocations: crate::util::SparseSlotMap::new(0),
        })
    }

    fn get_handle(&self) -> &Self::HandleType {
        self.buffer.get_handle()
    }

    fn handle(&self) -> Self::HandleType {
        self.buffer.handle()
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        self.buffer.get_device()
    }
}

impl<A: Allocator> SuballocatedBuffer<A> {
    pub fn create_suballocation(
        &mut self,
        offset: vk::DeviceSize,
        length: vk::DeviceSize,
    ) -> Result<Slot<Suballocation<A>>> {
        if self.buffer.get_size() < offset + length {
            return Err(anyhow::Error::from(DagalError::InsufficientSpace));
        }
        Ok(self.suballocations.insert(Suballocation {
            buffer: Arc::downgrade(&self.buffer),
            mapped_ptr: self.buffer.mapped_ptr(),
            offset,
            length,
        }))
    }
}
