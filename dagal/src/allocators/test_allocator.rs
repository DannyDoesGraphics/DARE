#![allow(dead_code)]

use crate::allocators::{Allocation, Allocator, MemoryLocation};
use crate::device::LogicalDevice;
use ash::vk::{DeviceMemory, DeviceSize, MemoryRequirements};
use std::ffi::c_void;
use std::ptr::NonNull;

/// An allocator with zero functionality for testing purposes only
/// You should only use this allocator if you know you don't need to rely on the allocator's
/// functionality
#[derive(Clone, Debug)]
pub struct TestAllocator {}

impl Allocator for TestAllocator {
    type Allocation = TestAllocation;

    fn allocate(
        &mut self,
        name: &str,
        requirements: &MemoryRequirements,
        ty: MemoryLocation,
    ) -> Result<Self::Allocation, crate::DagalError> {
        unimplemented!()
    }

    fn free(&mut self, allocation: Self::Allocation) -> Result<(), crate::DagalError> {
        unimplemented!()
    }

    fn get_device(&self) -> &LogicalDevice {
        todo!()
    }

    fn device(&self) -> LogicalDevice {
        todo!()
    }
}

#[derive(Debug, Default)]
pub struct TestAllocation {}

impl Allocation for TestAllocation {
    fn memory(&self) -> DeviceMemory {
        unimplemented!()
    }

    fn offset(&self) -> DeviceSize {
        unimplemented!()
    }

    fn mapped_ptr(&self) -> Option<NonNull<c_void>> {
        unimplemented!()
    }

    fn name(&self) -> &str {
        unimplemented!()
    }
}
