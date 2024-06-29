use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use ash::vk;
use ash::vk::{DeviceMemory, DeviceSize, MemoryRequirements};

use crate::allocators::Allocator;
use crate::traits::Destructible;

#[derive(Clone)]
pub struct GPUAllocatorImpl {
    handle: Arc<RwLock<Option<gpu_allocator::vulkan::Allocator>>>,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    buffer_device_address: bool,
}

impl Destructible for GPUAllocatorImpl {
    fn destroy(&mut self) {
        let mut guard = self.handle.write().unwrap();
        if let Some(handle) = guard.take() {
            drop(handle)
        }
    }
}

impl GPUAllocatorImpl {
    pub fn new(allocator_ci: gpu_allocator::vulkan::AllocatorCreateDesc) -> Result<Self> {
        let handle = gpu_allocator::vulkan::Allocator::new(&allocator_ci)?;

        Ok(Self {
            handle: Arc::new(RwLock::new(Some(handle))),
            memory_properties: Default::default(),
            buffer_device_address: allocator_ci.buffer_device_address,
        })
    }

    fn free_impl(&self, mut allocation: <GPUAllocatorImpl as Allocator>::Allocation) -> Result<()> {
        let mut guard = self
            .handle
            .write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        if let Some(handle) = allocation.handle.take() {
            #[cfg(feature = "log-lifetimes")]
            tracing::trace!("Destroying VkMemory {:p}", unsafe { handle.memory() });
            guard.as_mut().unwrap().free(handle)?;
        }
        Ok(())
    }
}

impl Allocator for GPUAllocatorImpl {
    type Allocation = GPUAllocatorAllocation;

    fn allocate(
        &mut self,
        name: &str,
        requirements: &MemoryRequirements,
        ty: super::MemoryLocation,
    ) -> Result<Self::Allocation> {
        let mut guard = self
            .handle
            .write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let allocate_ci = gpu_allocator::vulkan::AllocationCreateDesc {
            name,
            requirements: *requirements,
            location: match ty {
                super::MemoryLocation::GpuOnly => gpu_allocator::MemoryLocation::GpuOnly,
                super::MemoryLocation::CpuToGpu => gpu_allocator::MemoryLocation::CpuToGpu,
                super::MemoryLocation::GpuToCpu => gpu_allocator::MemoryLocation::GpuToCpu,
                super::MemoryLocation::CpuOnly => gpu_allocator::MemoryLocation::Unknown,
            },
            linear: false,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
        };
        let handle = guard.as_mut().unwrap().allocate(&allocate_ci)?;
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Creating VkMemory {:p}", unsafe { handle.memory() });

        Ok(GPUAllocatorAllocation {
            handle: Some(handle),
            name: name.to_string(),
        })
    }

    fn free(&mut self, allocation: Self::Allocation) -> Result<()> {
        self.free_impl(allocation)
    }
}

#[derive(Default, Debug)]
pub struct GPUAllocatorAllocation {
    handle: Option<gpu_allocator::vulkan::Allocation>,
    name: String,
}

impl super::Allocation for GPUAllocatorAllocation {
    fn memory(&self) -> DeviceMemory {
        unsafe { self.handle.as_ref().unwrap().memory() }
    }

    fn offset(&self) -> DeviceSize {
        self.handle.as_ref().unwrap().offset()
    }

    fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        self.handle.as_ref().unwrap().mapped_ptr()
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
}
