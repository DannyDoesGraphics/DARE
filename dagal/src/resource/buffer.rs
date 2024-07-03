use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr::NonNull;
use std::{mem, ptr};

use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use derivative::Derivative;

use crate::allocators::{Allocator, ArcAllocation, ArcAllocator, GPUAllocatorImpl};
use crate::command::command_buffer::CmdBuffer;
use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;
use crate::util::immediate_submit::ImmediateSubmitContext;

#[derive(Derivative, Debug)]
pub struct Buffer<A: Allocator = GPUAllocatorImpl> {
    handle: vk::Buffer,
    device: crate::device::LogicalDevice,
    allocation: Option<ArcAllocation<A>>,
    address: vk::DeviceAddress,
    size: vk::DeviceSize,
    name: Option<String>,
}

pub enum BufferCreateInfo<'a, A: Allocator = GPUAllocatorImpl> {
    /// Create a buffer with a new empty buffer with the requested size
    NewEmptyBuffer {
        device: crate::device::LogicalDevice,
        allocator: &'a mut ArcAllocator<A>,
        size: vk::DeviceSize,
        memory_type: crate::allocators::MemoryLocation,
        usage_flags: vk::BufferUsageFlags,
    },
}

impl<A: Allocator> Destructible for Buffer<A> {
    fn destroy(&mut self) {
        unsafe {
            #[cfg(feature = "log-lifetimes")]
            trace!("Destroying VkBuffer {:p}", self.handle);

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

    /// Upload data to a buffer with basic safety ensured.
    ///
    /// We currently only check if the buffer is smaller
    pub fn upload<T: Sized>(
        &mut self,
        immediate: &mut crate::util::ImmediateSubmit,
        allocator: &mut ArcAllocator<A>,
        content: &[T],
    ) -> Result<()> {
        if (mem::size_of_val(content) as vk::DeviceSize) > self.size {
            return Err(anyhow::Error::from(crate::DagalError::InsufficientSpace));
        }
        unsafe { self.upload_arbitrary::<T>(immediate, allocator, content) }
    }

    /// Upload arbitrary data to a buffer without any form of safety checking
    ///
    /// # Safety
    /// We do not make guarantees the type you're uploading fits inside the buffer
    pub unsafe fn upload_arbitrary<T: Sized>(
        &mut self,
        immediate: &mut crate::util::ImmediateSubmit,
        allocator: &mut ArcAllocator<A>,
        content: &[T],
    ) -> Result<()> {
        let buffer_size: vk::DeviceSize = mem::size_of_val(content) as vk::DeviceSize;
        let staging_buffer = Self::new(BufferCreateInfo::NewEmptyBuffer {
            device: self.device.clone(),
            allocator,
            size: buffer_size,
            memory_type: crate::allocators::MemoryLocation::CpuToGpu,
            usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
        })?;
        unsafe {
            ptr::copy_nonoverlapping::<u8>(
                content.as_ptr() as *const u8,
                staging_buffer.mapped_ptr().unwrap().as_ptr() as *mut u8,
                buffer_size as usize,
            );
        }
        {
            immediate.submit(Box::new({
                let src_buffer = staging_buffer.handle();
                let dst_buffer = self.handle();
                move |context: ImmediateSubmitContext| {
                    let copy = vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: buffer_size,
                    };
                    unsafe {
                        context.device.get_handle().cmd_copy_buffer(
                            context.cmd.handle(),
                            src_buffer,
                            dst_buffer,
                            &[copy],
                        );
                    }
                }
            }))?;
        }
        drop(staging_buffer);
        Ok(())
    }

    /// Write to a mapped pointer if one exists
    ///
    /// Offset is in bytes
    pub fn write<T: Sized>(&mut self, offset_bytes: vk::DeviceSize, data: &[T]) -> Result<()> {
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

impl<'a, A: Allocator + 'a> Resource<'a> for Buffer<A> {
    type CreateInfo = BufferCreateInfo<'a, A>;
    type HandleType = vk::Buffer;
    fn new(create_info: Self::CreateInfo) -> Result<Self> {
        return match create_info {
            BufferCreateInfo::NewEmptyBuffer {
                device,
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
                            sharing_mode: if device.get_used_queue_families().len() == 1 {
                                vk::SharingMode::EXCLUSIVE
                            } else {
                                vk::SharingMode::CONCURRENT
                            },
                            queue_family_index_count: if device.get_used_queue_families().len() == 1
                            {
                                0
                            } else {
                                device.get_used_queue_families().len() as u32
                            },
                            p_queue_family_indices: if device.get_used_queue_families().len() == 1 {
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

                Ok(Self {
                    handle,
                    device,
                    allocation: Some(allocation),
                    address,
                    size,
                    name: None,
                })
            }
        };
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

impl<A: Allocator> Nameable for Buffer<A> {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::BUFFER;
    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        self.name = Some(name.to_string());
        Ok(())
    }
}
