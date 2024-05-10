use std::sync::{Arc, RwLock};

use anyhow::Result;
use ash::vk;

use crate::allocators::{Allocation, Allocator};
use crate::traits::Destructible;
use crate::util::slot_map::{Slot, SlotMap};

/// Instead of handing out allocations themselves, we opt to hand out handles to allocations,
/// thus allowing for cloning of structs which may have allocations
#[derive(Debug)]
pub struct SlotMapMemoryAllocator<T: Allocator> {
    allocator: T,
    slot_map: Arc<RwLock<SlotMap<T::Allocation>>>,
}

impl<T: Allocator> Clone for SlotMapMemoryAllocator<T> {
    fn clone(&self) -> Self {
        Self {
            allocator: self.allocator.clone(),
            slot_map: self.slot_map.clone(),
        }
    }
}

impl<T: Allocator> SlotMapMemoryAllocator<T> {
    /// Create a new slot map memory allocator
    pub fn new(allocator: T) -> Self {
        Self {
            allocator,
            slot_map: Arc::new(RwLock::new(SlotMap::new())),
        }
    }

    /// Allocate memory and store it in a slot map and get a handle to it
    pub fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: super::MemoryType,
    ) -> Result<MemoryAllocation<T>> {
        let allocation = self.allocator.allocate(name, requirements, ty)?;
        let mut slot_map = self
            .slot_map
            .try_write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let handle = slot_map.insert(allocation);
        Ok(MemoryAllocation {
            handle,
            slot_map: self.clone(),
        })
    }

    /// Deallocate memory using a handle
    pub fn free(&mut self, allocation: Slot<T::Allocation>) -> Result<()> {
        let mut slot_map = self
            .slot_map
            .try_write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let allocation = slot_map.try_lock_erase(allocation)?;
        self.allocator.free(allocation)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MemoryAllocation<T: Allocator> {
    handle: Slot<T::Allocation>,
    slot_map: SlotMapMemoryAllocator<T>,
}

impl<T: Allocator> PartialEq for MemoryAllocation<T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<A: Allocator> Clone for MemoryAllocation<A> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            slot_map: self.slot_map.clone(),
        }
    }
}

unsafe impl<T: Allocator> Send for MemoryAllocation<T> {}
unsafe impl<T: Allocator> Sync for MemoryAllocation<T> {}

impl<T: Allocator> Destructible for MemoryAllocation<T> {
    fn destroy(&mut self) {
        self.slot_map.free(self.handle.clone()).unwrap()
    }
}

impl<T: Allocator> MemoryAllocation<T> {
    /// Get the underlying memory offset
    pub fn offset(&self) -> Result<vk::DeviceSize> {
        let slot_map = self
            .slot_map
            .slot_map
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let handle = slot_map.get(&self.handle)?;
        Ok(handle.offset())
    }

    /// Get the underlying memory
    pub fn memory(&self) -> Result<vk::DeviceMemory> {
        let slot_map = self
            .slot_map
            .slot_map
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let handle = slot_map.get(&self.handle)?;
        Ok(handle.memory())
    }
}
