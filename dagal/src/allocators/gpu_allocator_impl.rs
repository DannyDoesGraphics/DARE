/// Mostly taken from https://github.com/NotAPenguin0/phobos-rs/blob/master/src/allocator/default_allocator.rs
///
/// Implements [`gpu_allocator`]
use std::ffi::c_void;

use ash::vk;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
// Implementation for gpu_allocator of the generic allocator object
use anyhow::Result;
use gpu_allocator;

#[derive(Clone)]
pub struct GpuAllocator {
    handle: Arc<Mutex<Option<gpu_allocator::vulkan::Allocator>>>,
}

#[derive(Default)]
pub struct GpuAllocation {
    allocator: Option<GpuAllocator>,
    handle: Option<gpu_allocator::vulkan::Allocation>,
}

impl Drop for GpuAllocation {
    fn drop(&mut self) {
        let mut allocator = self.allocator.clone().unwrap();
        allocator.free_impl(self).unwrap();
    }
}

unsafe impl Send for GpuAllocator {}
unsafe impl Sync for GpuAllocator {}

impl super::Allocator for GpuAllocator {
    type Allocation = GpuAllocation;

    fn allocate(
        &mut self,
        _name: &str,
        _requirements: &vk::MemoryRequirements,
        _ty: super::MemoryType,
    ) -> Result<GpuAllocation> {
        /*
        self.handle.lock()?.allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
            name,
            requirements: *requirements,
            location: gpu_allocator::MemoryLocation::from(ty),
            linear: false,
            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
        });
        */
        unimplemented!()
    }

    fn free(&mut self, mut allocation: Self::Allocation) -> Result<()> {
        self.free_impl(&mut allocation)
    }
}

impl GpuAllocator {
    pub fn new(_instance: ash::Instance, _device: ash::Device) -> Self {
        /*
        let gpu_allocator = gpu_allocator::vulkan::Allocator::new(&gpu_allocator::vulkan::AllocatorCreateDesc {
            instance,
            device,
            physical_device: Default::default(),
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        });
        Self {
            handle: Arc::new(Mutex::new()),
        }
        */
        unimplemented!()
    }

    fn free_impl(&mut self, allocation: &mut <Self as super::Allocator>::Allocation) -> Result<()> {
        let mut allocator = self
            .handle
            .lock()
            .map_err(|_| crate::DagalError::PoisonError)?;
        match allocation.handle.take() {
            None => {}
            Some(allocation) => {
                allocator.as_mut().unwrap().free(allocation)?;
            }
        }
        Ok(())
    }
}

impl super::Allocation for GpuAllocation {
    fn memory(&self) -> vk::DeviceMemory {
        //unsafe { self.handle.memory() as vk::DeviceMemory }
        panic!("Currently unsupported.")
    }

    fn offset(&self) -> vk::DeviceSize {
        self.handle.as_ref().unwrap().offset()
    }

    fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        self.handle.as_ref().unwrap().mapped_ptr()
    }

    fn name(&self) -> &str {
        unimplemented!()
    }
}

impl From<super::MemoryType> for gpu_allocator::MemoryLocation {
    fn from(value: super::MemoryType) -> Self {
        match value {
            super::MemoryType::GpuOnly => gpu_allocator::MemoryLocation::GpuOnly,
            super::MemoryType::CpuToGpu => gpu_allocator::MemoryLocation::CpuToGpu,
            super::MemoryType::GpuToCpu => gpu_allocator::MemoryLocation::GpuToCpu,
        }
    }
}
