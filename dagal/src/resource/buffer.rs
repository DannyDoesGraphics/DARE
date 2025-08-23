use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use derivative::Derivative;
use std::ffi::c_void;
use std::hash::Hasher;
use std::ptr::NonNull;
use std::{mem, ptr};

use crate::allocators::{Allocator, ArcAllocation, ArcAllocator};
use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer<A: Allocator> {
    handle: vk::Buffer,
    device: crate::device::LogicalDevice,
    #[derivative(Debug = "ignore")]
    allocation: Option<ArcAllocation<A>>,
    address: vk::DeviceAddress,
    size: vk::DeviceSize,
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

/// Similar to [`vk::BufferCreateInfo`], but supports hashing + cloning, but restrictive in regards to extensions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnedBufferCreateInfo {
    pub name: Option<String>,
    pub location: crate::allocators::MemoryLocation,
    pub flags: vk::BufferCreateFlags,
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub sharing_mode: vk::SharingMode,
    pub queue_family_indices: Vec<u32>,
}

pub enum BufferCreateInfo<'a, A: Allocator> {
    /// Create a buffer with a new empty buffer with the requested size
    NewEmptyBuffer {
        device: crate::device::LogicalDevice,
        name: Option<String>,
        allocator: &'a mut ArcAllocator<A>,
        size: vk::DeviceSize,
        memory_type: crate::allocators::MemoryLocation,
        usage_flags: vk::BufferUsageFlags,
    },
    /// Create a buffer with an existing memory allocation
    NewBufferWithAllocation {
        device: crate::device::LogicalDevice,
        name: Option<String>,
        allocator: &'a mut ArcAllocator<A>,
        size: vk::DeviceSize,
        allocation: ArcAllocation<A>,
        usage_flags: vk::BufferUsageFlags,
    },
    FromOwnedCreateInfo{
        create_info: OwnedBufferCreateInfo,
        device: crate::device::LogicalDevice,
        allocator: &'a mut ArcAllocator<A>,
    }
}

impl<A: Allocator> Destructible for Buffer<A> {
    fn destroy(&mut self) {
        unsafe {
            #[cfg(feature = "log-lifetimes")]
            tracing::trace!("Destroying VkBuffer {:p}", self.handle);

            self.device.get_handle().destroy_buffer(self.handle, None);
            if let Some(mut allocation) = self.allocation.take() {
                allocation.destroy();
            }
        }
    }
}

#[cfg(feature = "raii")]
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

    /// Acquire a mapped pointer to the buffer allocation
    pub fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        match self.allocation.as_ref() {
            None => None,
            Some(allocation) => allocation.mapped_ptr().unwrap(),
        }
    }

    /// Write to a mapped pointer if one exists
    ///
    /// Offset is in bytes
    pub fn write<T: Sized>(&mut self, offset_bytes: vk::DeviceSize, data: &[T]) -> Result<()> {
        if offset_bytes + (size_of_val(data) as vk::DeviceSize) > self.size {
            return Err(anyhow::Error::from(crate::DagalError::InsufficientSpace));
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
            Err(anyhow::Error::from(crate::DagalError::NoMappedPointer))
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
                let handle = unsafe {
                    device.get_handle().create_buffer(
                        &vk::BufferCreateInfo {
                            s_type: vk::StructureType::BUFFER_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::BufferCreateFlags::empty(),
                            size,
                            usage: usage_flags,
                            sharing_mode: if device.get_used_queue_families().len() <= 1 {
                                vk::SharingMode::EXCLUSIVE
                            } else {
                                vk::SharingMode::CONCURRENT
                            },
                            queue_family_index_count: if device.get_used_queue_families().len() <= 1
                            {
                                0
                            } else {
                                device.get_used_queue_families().len() as u32
                            },
                            p_queue_family_indices: if device.get_used_queue_families().len() <= 1 {
                                ptr::null()
                            } else {
                                device.get_used_queue_families().as_ptr()
                            },
                            _marker: Default::default(),
                        },
                        None,
                    )?
                };
                let mem_requirements =
                    unsafe { device.get_handle().get_buffer_memory_requirements(handle) };
                let allocation = allocator.allocate("buffer", &mem_requirements, memory_type)?;
                unsafe {
                    device.get_handle().bind_buffer_memory(
                        handle,
                        allocation.memory()?,
                        allocation.offset()?,
                    )?
                }
                let mut address = vk::DeviceAddress::default();
                if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    == vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                {
                    address = unsafe {
                        device.get_handle().get_buffer_device_address(
                            &vk::BufferDeviceAddressInfo {
                                s_type: vk::StructureType::BUFFER_DEVICE_ADDRESS_INFO,
                                p_next: ptr::null(),
                                buffer: handle,
                                _marker: Default::default(),
                            },
                        )
                    };
                }
                let mut buffer = Self {
                    handle,
                    device: device.clone(),
                    allocation: Some(allocation),
                    address,
                    size,
                    name: name.clone(),
                };

                if let (Some(debug_utils), Some(name)) = (device.get_debug_utils(), name) {
                    buffer.set_name(debug_utils, &name)?;
                }

                Ok(buffer)
            },
            BufferCreateInfo::FromOwnedCreateInfo { create_info, device, allocator } => {
                let handle = unsafe {
                    device.get_handle().create_buffer(
                        &vk::BufferCreateInfo {
                            s_type: vk::StructureType::BUFFER_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: create_info.flags,
                            size: create_info.size,
                            usage: create_info.usage,
                            sharing_mode: create_info.sharing_mode,
                            queue_family_index_count: create_info.queue_family_indices.len() as u32,
                            p_queue_family_indices: create_info.queue_family_indices.as_ptr(),
                            _marker: Default::default(),
                        },
                        None,
                    )?
                };
                let mem_requirements =
                    unsafe { device.get_handle().get_buffer_memory_requirements(handle) };
                let allocation = allocator.allocate("buffer", &mem_requirements, create_info.location)?;
                unsafe {
                    device.get_handle().bind_buffer_memory(
                        handle,
                        allocation.memory()?,
                        allocation.offset()?,
                    )?
                }
                let mut address = vk::DeviceAddress::default();
                if create_info.usage & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    == vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                {
                    address = unsafe {
                        device.get_handle().get_buffer_device_address(
                            &vk::BufferDeviceAddressInfo {
                                s_type: vk::StructureType::BUFFER_DEVICE_ADDRESS_INFO,
                                p_next: ptr::null(),
                                buffer: handle,
                                _marker: Default::default(),
                            },
                        )
                    };
                }
                let mut buffer = Self {
                    handle,
                    device: device.clone(),
                    allocation: Some(allocation),
                    address,
                    size: create_info.size,
                    name: None,
                };
                if let (Some(debug_utils), Some(name)) = (device.get_debug_utils(), &create_info.name) {
                    buffer.set_name(debug_utils, name)?;
                }

                Ok(buffer)
            },
            _ => unimplemented!(),
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
    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<(), crate::DagalError> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        self.name = Some(name.to_string());
        Ok(())
    }
}
