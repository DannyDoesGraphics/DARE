use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use crate::allocators::MemoryLocation;
use anyhow::Result;
use ash::vk;
use derivative::Derivative;
use tracing::trace;
use vk_mem::Alloc;

use crate::traits::Destructible;

#[derive(Clone)]
pub struct VkMemAllocator {
    handle: Arc<Mutex<Option<vk_mem::Allocator>>>,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    buffer_device_address: bool,
}

unsafe impl Send for VkMemAllocator {}
unsafe impl Sync for VkMemAllocator {}

impl Destructible for VkMemAllocator {
    fn destroy(&mut self) {
        if let Some(allocator) = self.handle.lock().unwrap().take() {
            drop(allocator)
        }
    }
}

impl VkMemAllocator {
    pub fn new(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        buffer_device_address: bool,
    ) -> Result<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let mut allocator_ci = vk_mem::AllocatorCreateInfo::new(instance, device, physical_device);
        if buffer_device_address {
            allocator_ci.flags = vk_mem::AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
        }
        Ok(Self {
            handle: unsafe { Arc::new(Mutex::new(Some(vk_mem::Allocator::new(allocator_ci)?))) },
            memory_properties,
            buffer_device_address,
        })
    }

    fn free_impl(&self, allocation: &mut <Self as super::Allocator>::Allocation) -> Result<()> {
        let allocator = self
            .handle
            .lock()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        match allocation.handle.take() {
            None => {}
            Some(mut allocation) => unsafe {
                #[cfg(feature = "log-lifetimes")]
                trace!("Destroying VkDeviceMemory");
                allocator.as_ref().unwrap().free_memory(&mut allocation);
            },
        }
        Ok(())
    }

    fn find_memory_type_index(
        &self,
        memory_type_bits: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        let memory_properties = self.memory_properties;

        for (index, memory_type) in memory_properties.memory_types.iter().enumerate() {
            if (memory_type_bits & (1 << index)) != 0
                && (memory_type.property_flags & properties) == properties
            {
                return Some(index as u32);
            }
        }
        None
    }
}

impl super::Allocator for VkMemAllocator {
    type Allocation = VkMemAllocation;

    fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: MemoryLocation,
    ) -> Result<Self::Allocation> {
        let allocator = self
            .handle
            .lock()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let allocator = allocator.as_ref().unwrap();
        let memory_property_flags = match ty {
            MemoryLocation::GpuOnly => vk::MemoryPropertyFlags::DEVICE_LOCAL,
            MemoryLocation::CpuToGpu => vk::MemoryPropertyFlags::HOST_VISIBLE,
            MemoryLocation::GpuToCpu => {
                vk::MemoryPropertyFlags::DEVICE_LOCAL
                    | vk::MemoryPropertyFlags::HOST_COHERENT
                    | vk::MemoryPropertyFlags::HOST_CACHED
            }
            MemoryLocation::CpuOnly => vk::MemoryPropertyFlags::empty(),
        };

        let memory_type_bits = self
            .find_memory_type_index(requirements.memory_type_bits, memory_property_flags)
            .unwrap();
        let allocation =
            unsafe {
                allocator.allocate_memory(
                    requirements,
                    &vk_mem::AllocationCreateInfo {
                        flags: match ty {
                            MemoryLocation::GpuOnly => vk_mem::AllocationCreateFlags::empty(),
                            MemoryLocation::CpuToGpu => {
                                vk_mem::AllocationCreateFlags::MAPPED
                                    | vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE
                            }
                            MemoryLocation::GpuToCpu => vk_mem::AllocationCreateFlags::MAPPED
                                | vk_mem::AllocationCreateFlags::HOST_ACCESS_ALLOW_TRANSFER_INSTEAD,
                            MemoryLocation::CpuOnly => {
                                vk_mem::AllocationCreateFlags::MAPPED
                                    | vk_mem::AllocationCreateFlags::HOST_ACCESS_RANDOM
                            }
                        },
                        required_flags: memory_property_flags,
                        usage: vk_mem::MemoryUsage::from(ty),
                        preferred_flags: vk::MemoryPropertyFlags::empty(),
                        memory_type_bits,
                        user_data: 0,
                        priority: 1.0,
                    },
                )?
            };
        let ai = allocator.get_allocation_info(&allocation);
        #[cfg(feature = "log-memory-allocations")]
        trace!("Creating memory allocation {:p}", ai.device_memory);

        Ok(VkMemAllocation {
            handle: Some(allocation),
            allocation_info: Some(ai),
            memory_requirements: *requirements,
            name: String::from(name),
        })
    }

    fn free(&mut self, mut allocation: Self::Allocation) -> Result<()> {
        self.free_impl(&mut allocation)
    }
}

/// Represents an allocation using [`vk_mem`]
///
/// By default, all allocations will automatically clean up after themselves after being used.
#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct VkMemAllocation {
    handle: Option<vk_mem::Allocation>,
    allocation_info: Option<vk_mem::AllocationInfo>,
    memory_requirements: vk::MemoryRequirements,
    name: String,
}

impl VkMemAllocation {
    /// Make a new [`VkMemAllocation`] from an already existing [`vk_mem::Allocation`]
    pub fn from_allocation(
        allocation: vk_mem::Allocation,
        allocator: VkMemAllocator,
        name: &str,
    ) -> Result<Self> {
        let ai: vk_mem::AllocationInfo;
        {
            let allocator_guard = allocator
                .handle
                .lock()
                .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
            let allocator_guard = allocator_guard.as_ref().unwrap();
            ai = allocator_guard.get_allocation_info(&allocation);
        }
        Ok(Self {
            handle: Some(allocation),
            allocation_info: Some(ai),
            memory_requirements: Default::default(),
            name: name.to_string(),
        })
    }
}

unsafe impl Send for VkMemAllocation {}
unsafe impl Sync for VkMemAllocation {}

impl super::Allocation for VkMemAllocation {
    fn memory(&self) -> vk::DeviceMemory {
        self.allocation_info.as_ref().unwrap().device_memory
    }

    fn offset(&self) -> vk::DeviceSize {
        let allocation_info = self.allocation_info.as_ref().unwrap();
        (allocation_info.offset + self.memory_requirements.alignment - 1)
            & !(self.memory_requirements.alignment - 1)
    }

    fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.allocation_info.as_ref().unwrap().mapped_data)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[allow(deprecated)]
impl From<MemoryLocation> for vk_mem::MemoryUsage {
    fn from(value: MemoryLocation) -> Self {
        match value {
            MemoryLocation::GpuOnly => vk_mem::MemoryUsage::GpuOnly,
            MemoryLocation::CpuToGpu => vk_mem::MemoryUsage::CpuToGpu,
            MemoryLocation::GpuToCpu => vk_mem::MemoryUsage::GpuToCpu,
            MemoryLocation::CpuOnly => vk_mem::MemoryUsage::CpuOnly,
        }
    }
}
