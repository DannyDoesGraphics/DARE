use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use derivative::Derivative;
use std::ffi::c_void;
use std::hash::Hasher;
use std::ptr::NonNull;
use std::{mem, ptr};

use crate::allocators::{Allocation, Allocator};
use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer<A: Allocator> {
    handle: vk::Buffer,
    device: crate::device::LogicalDevice,
    #[derivative(Debug = "ignore")]
    allocation: Option<A::Allocation>,
    #[derivative(Debug = "ignore")]
    allocator: Option<A>,
    address: vk::DeviceAddress,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    name: Option<String>,
}
unsafe impl<A: Allocator> Send for Buffer<A> {}

impl<A: Allocator> PartialEq for Buffer<A> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<A: Allocator> Eq for Buffer<A> {}

impl<A: Allocator> std::hash::Hash for Buffer<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub location: crate::allocators::MemoryLocation,
}

pub enum BufferCreateInfo<'a, A: Allocator> {
    /// Create a buffer with a new empty buffer with the requested size
    NewEmptyBuffer {
        device: crate::device::LogicalDevice,
        name: Option<String>,
        allocator: &'a A,
        /// Size in bytes
        size: vk::DeviceSize,
        memory_type: crate::allocators::MemoryLocation,
        usage_flags: vk::BufferUsageFlags,
    },
    /// Create a buffer without any allocation bound
    NewUnallocated {
        device: crate::device::LogicalDevice,
        size: vk::DeviceSize,
        usage_flags: vk::BufferUsageFlags,
    },
}

impl<A: Allocator> Destructible for Buffer<A> {
    fn destroy(&mut self) {
        unsafe {
            #[cfg(feature = "log-lifetimes")]
            log::trace!("Destroying VkBuffer {:p}", self.handle);

            self.device.get_handle().destroy_buffer(self.handle, None);
            if let Some(allocation) = self.allocation.take()
                && let Some(allocator) = self.allocator.as_mut()
            {
                let _ = allocator.free(allocation);
            }
        }
    }
}

impl<A: Allocator> Drop for Buffer<A> {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl<A: Allocator> Buffer<A> {
    /// If BDA is enabled, you are able to acquire the [`VkDeviceAddress`](vk::DeviceAddress) of the
    /// buffer
    pub fn address(&self) -> vk::DeviceAddress {
        self.address
    }

    pub fn bind_memory(&mut self, allocation: A::Allocation) -> Result<(), crate::DagalError> {
        assert!(
            self.allocation.is_none(),
            "buffer is already bound to memory"
        );
        unsafe {
            self.device.get_handle().bind_buffer_memory(
                self.handle,
                allocation.memory(),
                allocation.offset(),
            )?;
        }
        if self.usage & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            == vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
        {
            self.address = unsafe {
                self.device.get_handle().get_buffer_device_address(
                    &vk::BufferDeviceAddressInfo::default().buffer(self.handle),
                )
            };
        }
        self.allocation = Some(allocation);
        Ok(())
    }

    #[must_use = "the returned allocation must be freed via its allocator or it will leak"]
    pub fn into_allocation(mut self) -> Option<A::Allocation> {
        self.allocation.take()
    }

    /// Acquire a mapped pointer to the buffer allocation
    pub fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        self.allocation
            .as_ref()
            .and_then(|allocation| allocation.mapped_ptr())
    }

    /// Read to a mapper pointer if one exists
    pub fn read<T: Sized>(
        &self,
        offset_bytes: vk::DeviceSize,
        amount: u64,
    ) -> Result<&[T], crate::DagalError> {
        if offset_bytes + (std::mem::size_of::<T>() as vk::DeviceSize * amount) > self.size {
            return Err(crate::DagalError::InsufficientSpace);
        }
        if let Some(mapped_ptr) = self.mapped_ptr() {
            // SAFETY: Known that size_of_val(data) + offset < buffer.size
            unsafe {
                let mapped_ptr = mapped_ptr.as_ptr().add(offset_bytes as usize) as *const T;
                Ok(std::slice::from_raw_parts(mapped_ptr, amount as usize))
            }
        } else {
            Err(crate::DagalError::NoMappedPointer)
        }
    }

    /// Write to a mapped pointer if one exists
    ///
    /// Offset is in bytes
    pub fn write<T: Sized>(
        &mut self,
        offset_bytes: vk::DeviceSize,
        data: &[T],
    ) -> Result<(), crate::DagalError> {
        if offset_bytes + (size_of_val(data) as vk::DeviceSize) > self.size {
            return Err(crate::DagalError::InsufficientSpace);
        }
        if let Some(mapped_ptr) = self.mapped_ptr() {
            // SAFETY: Known that size_of_val(data) + offset < buffer.size
            unsafe {
                let data_ptr = data.as_ptr() as *const _ as *const c_void;
                let mapped_ptr = mapped_ptr.as_ptr().add(offset_bytes as usize);
                ptr::copy_nonoverlapping(data_ptr, mapped_ptr, size_of_val(data));
            }
            Ok(())
        } else {
            Err(crate::DagalError::NoMappedPointer)
        }
    }

    /// Write to a mapped pointer if one exists
    ///
    /// Offset is in bytes
    pub unsafe fn write_unsafe<T: Sized>(
        &self,
        offset_bytes: vk::DeviceSize,
        data: &[T],
    ) -> Result<()> {
        if offset_bytes + (mem::size_of_val(data) as vk::DeviceSize) > self.size {
            return Err(anyhow::Error::from(crate::DagalError::InsufficientSpace));
        }
        if let Some(mapped_ptr) = self.mapped_ptr() {
            // SAFETY: Known that size_of_val(data) + offset < buffer.size
            unsafe {
                let data_ptr = data.as_ptr() as *const _ as *const c_void;
                let mapped_ptr = mapped_ptr.as_ptr().add(offset_bytes as usize);
                ptr::copy_nonoverlapping(data_ptr, mapped_ptr, mem::size_of_val(data));
            }
            Ok(())
        } else {
            Err(anyhow::Error::from(crate::DagalError::NoMappedPointer))
        }
    }

    pub fn get_size(&self) -> vk::DeviceSize {
        self.size
    }
}

impl<A: Allocator> crate::resource::traits::Buildable for Buffer<A> {
    type Desc = BufferDesc;
    type Alloc = A;

    fn build(
        desc: &Self::Desc,
        device: &crate::device::LogicalDevice,
        allocator: &Self::Alloc,
    ) -> Result<Self, crate::DagalError> {
        Self::new(BufferCreateInfo::NewEmptyBuffer {
            device: device.clone(),
            name: None,
            allocator,
            size: desc.size,
            memory_type: desc.location,
            usage_flags: desc.usage,
        })
    }
}

impl<A: Allocator> Resource for Buffer<A> {
    type CreateInfo<'a> = BufferCreateInfo<'a, A>;
    fn new(create_info: Self::CreateInfo<'_>) -> Result<Self, crate::DagalError> {
        match create_info {
            BufferCreateInfo::NewEmptyBuffer {
                device,
                name,
                allocator,
                size,
                memory_type,
                usage_flags,
            } => {
                let queue_families = device.get_used_queue_families();
                let concurrent = queue_families.len() > 1;
                let mut buffer_ci = vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage_flags)
                    .sharing_mode(if concurrent {
                        vk::SharingMode::CONCURRENT
                    } else {
                        vk::SharingMode::EXCLUSIVE
                    });
                if concurrent {
                    buffer_ci = buffer_ci.queue_family_indices(queue_families);
                }
                let handle = unsafe { device.get_handle().create_buffer(&buffer_ci, None)? };
                let mem_requirements =
                    unsafe { device.get_handle().get_buffer_memory_requirements(handle) };
                let allocation = allocator.allocate("buffer", &mem_requirements, memory_type)?;
                unsafe {
                    device.get_handle().bind_buffer_memory(
                        handle,
                        allocation.memory(),
                        allocation.offset(),
                    )?
                }
                let mut address = vk::DeviceAddress::default();
                if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    == vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                {
                    address = unsafe {
                        device.get_handle().get_buffer_device_address(
                            &vk::BufferDeviceAddressInfo::default().buffer(handle),
                        )
                    };
                }
                let mut buffer = Self {
                    handle,
                    device: device.clone(),
                    allocation: Some(allocation),
                    allocator: Some(allocator.clone()),
                    address,
                    size,
                    usage: usage_flags,
                    name: name.clone(),
                };

                if let (Some(debug_utils), Some(name)) = (device.get_debug_utils(), name) {
                    buffer.set_name(debug_utils, &name)?;
                }

                Ok(buffer)
            }
            BufferCreateInfo::NewUnallocated {
                device,
                size,
                usage_flags,
            } => {
                let queue_families = device.get_used_queue_families();
                let concurrent = queue_families.len() > 1;
                let mut buffer_ci = vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage_flags)
                    .sharing_mode(if concurrent {
                        vk::SharingMode::CONCURRENT
                    } else {
                        vk::SharingMode::EXCLUSIVE
                    });
                if concurrent {
                    buffer_ci = buffer_ci.queue_family_indices(queue_families);
                }
                let handle = unsafe { device.get_handle().create_buffer(&buffer_ci, None)? };
                Ok(Self {
                    handle,
                    device,
                    allocation: None,
                    allocator: None,
                    address: vk::DeviceAddress::default(),
                    size,
                    usage: usage_flags,
                    name: None,
                })
            }
        }
    }
    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl<A: Allocator> AsRaw for Buffer<A> {
    type RawType = vk::Buffer;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}

impl<A: Allocator> Nameable for Buffer<A> {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::BUFFER;
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> Result<(), crate::DagalError> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        self.name = Some(name.to_string());
        Ok(())
    }
}
