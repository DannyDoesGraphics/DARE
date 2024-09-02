use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocation, Allocator, GPUAllocatorImpl};
use crate::traits::Destructible;

/// An ArcAllocator wraps all memory allocations in a `Arc<RwLock<Option<A::Allocation>>>` to allow
/// for A::Allocation to delete themselves
#[derive(Debug)]
pub struct ArcAllocator<A: Allocator = GPUAllocatorImpl> {
    allocator: A,
}

/// Simply holds an arc reference to the original data as well a reference to the allocator.
///
/// This main purpose of this is to allow the allocations to delete themselves.
#[derive(Debug)]
pub struct ArcAllocation<A: Allocator = GPUAllocatorImpl> {
    allocator: A,
    allocation: Arc<RwLock<Option<A::Allocation>>>,
}

impl<A: Allocator> ArcAllocator<A> {
    pub fn new(allocator: A) -> Self {
        Self { allocator }
    }

    pub fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: super::MemoryLocation,
    ) -> Result<ArcAllocation<A>> {
        let allocation = self.allocator.allocate(name, requirements, ty)?;
        Ok(ArcAllocation {
            allocator: self.allocator.clone(),
            allocation: Arc::new(RwLock::new(Some(allocation))),
        })
    }

    pub fn get_device(&self) -> &crate::device::LogicalDevice {
        self.allocator.get_device()
    }

    pub fn device(&self) -> crate::device::LogicalDevice {
        self.allocator.device()
    }
}

impl<A: Allocator> ArcAllocation<A> {
    pub fn offset(&self) -> Result<vk::DeviceSize> {
        self.allocation
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .as_ref()
            .map(|allocation| Ok(allocation.offset()))
            .unwrap_or_else(|| {
                Err(anyhow::Error::from(
                    crate::DagalError::EmptyMemoryAllocation,
                ))
            })
    }

    pub fn memory(&self) -> Result<vk::DeviceMemory> {
        self.allocation
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .as_ref()
            .map(|allocation| Ok(allocation.memory()))
            .unwrap_or_else(|| {
                Err(anyhow::Error::from(
                    crate::DagalError::EmptyMemoryAllocation,
                ))
            })
    }

    pub fn mapped_ptr(&self) -> Result<Option<NonNull<c_void>>> {
        self.allocation
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .as_ref()
            .map(|allocation| Ok(allocation.mapped_ptr()))
            .unwrap_or_else(|| {
                Err(anyhow::Error::from(
                    crate::DagalError::EmptyMemoryAllocation,
                ))
            })
    }
}

impl<A: Allocator> Destructible for ArcAllocation<A> {
    fn destroy(&mut self) {
        self.allocation
            .write()
            .map(|mut allocation| {
                allocation
                    .take()
                    .map(|allocation| self.allocator.free(allocation))
            })
            .unwrap();
    }
}

impl<A: Allocator> Clone for ArcAllocation<A> {
    fn clone(&self) -> Self {
        Self {
            allocator: self.allocator.clone(),
            allocation: self.allocation.clone(),
        }
    }
}

impl<A: Allocator> Clone for ArcAllocator<A> {
    fn clone(&self) -> Self {
        Self {
            allocator: self.allocator.clone(),
        }
    }
}

unsafe impl<A: Allocator> Send for ArcAllocation<A> {}

unsafe impl<A: Allocator> Sync for ArcAllocation<A> {}
