/// Mostly taken from https://github.com/NotAPenguin0/phobos-rs/blob/master/src/allocator/traits.rs
///
/// Provides traits for implementing allocators
use std::ffi::c_void;
use std::fmt::Debug;
use std::ptr::NonNull;

use anyhow::Result;
use ash::vk;

pub use arc_allocator::{ArcAllocation, ArcAllocator};
#[cfg(feature = "gpu-allocator")]
pub use gpu_allocator_impl::*;
pub use memory_type::*;

#[cfg(feature = "gpu-allocator")]
pub mod gpu_allocator_impl;

pub mod arc_allocator;
pub mod memory_type;
pub mod test_allocator;

/// An interface to universally interact with all allocators with
pub trait Allocator: Debug + Clone + Send + Sync + Unpin {
    type Allocation: Allocation;

    /// Create a new allocation
    fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: MemoryLocation,
    ) -> Result<Self::Allocation>;

    /// Free an allocation
    fn free(&mut self, allocation: Self::Allocation) -> Result<()>;

    /// Get device reference
    fn get_device(&self) -> &crate::device::LogicalDevice;

    /// Get device
    fn device(&self) -> crate::device::LogicalDevice;
}

pub trait Allocation: Default + Send + Sync + Debug {
    /// Get the underlying [`vk::DeviceMemory`]
    fn memory(&self) -> vk::DeviceMemory;

    /// Get the offset of the memory
    fn offset(&self) -> vk::DeviceSize;

    /// Get the raw ptr that underlies the allocation
    fn mapped_ptr(&self) -> Option<NonNull<c_void>>;
    /// Get name of the allocation
    fn name(&self) -> &str;
}
