use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

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
    ) -> Result<Self> {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        Ok(Self {
            handle: Arc::new(Mutex::new(Some(vk_mem::Allocator::new(
                vk_mem::AllocatorCreateInfo::new(instance, device, physical_device),
            )?))),
            memory_properties,
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
        memory_requirements: &vk::MemoryRequirements,
        flags: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        self.memory_properties
            .memory_types
            .iter()
            .find(|memory_type| {
                (1 << memory_type.heap_index) & memory_requirements.memory_type_bits != 0
                    && memory_type.property_flags.contains(flags)
            })
            .map(|memory_type| memory_type.heap_index as _)
    }
}

impl super::Allocator for VkMemAllocator {
    type Allocation = VkMemAllocation;

    fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: super::MemoryType,
    ) -> Result<Self::Allocation> {
        let allocator = self
            .handle
            .lock()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let allocator = allocator.as_ref().unwrap();
        let memory_property_flags = match ty {
            super::MemoryType::GpuOnly => vk::MemoryPropertyFlags::DEVICE_LOCAL,
            super::MemoryType::CpuToGpu => {
                vk::MemoryPropertyFlags::HOST_VISIBLE
                    | vk::MemoryPropertyFlags::HOST_COHERENT
                    | vk::MemoryPropertyFlags::DEVICE_LOCAL
            }
            super::MemoryType::GpuToCpu => {
                vk::MemoryPropertyFlags::DEVICE_LOCAL
                    | vk::MemoryPropertyFlags::HOST_COHERENT
                    | vk::MemoryPropertyFlags::HOST_CACHED
            }
        };

        let memory_type_bits = self
            .find_memory_type_index(requirements, memory_property_flags)
            .unwrap();
        let allocation = unsafe {
            allocator.allocate_memory(
                requirements,
                &vk_mem::AllocationCreateInfo {
                    flags: vk_mem::AllocationCreateFlags::empty(),
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
impl From<super::MemoryType> for vk_mem::MemoryUsage {
    fn from(value: super::MemoryType) -> Self {
        match value {
            crate::allocators::MemoryType::GpuOnly => vk_mem::MemoryUsage::GpuOnly,
            crate::allocators::MemoryType::CpuToGpu => vk_mem::MemoryUsage::CpuToGpu,
            crate::allocators::MemoryType::GpuToCpu => vk_mem::MemoryUsage::GpuToCpu,
        }
    }
}
