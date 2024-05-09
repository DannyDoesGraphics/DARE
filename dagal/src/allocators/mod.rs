use anyhow::Result;
use ash::vk;
/// Mostly taken from https://github.com/NotAPenguin0/phobos-rs/blob/master/src/allocator/traits.rs
///
/// Provides traits for implementing allocators
use std::ffi::c_void;
use std::ptr::NonNull;

#[cfg(feature = "gpu-allocator")]
pub mod gpu_allocator_impl;
#[cfg(feature = "gpu-allocator")]
pub use gpu_allocator_impl::*;
#[cfg(feature = "vk-mem-rs")]
pub mod vk_mem_impl;
#[cfg(feature = "vk-mem-rs")]
pub use vk_mem_impl::*;

pub mod memory_type;

pub use memory_type::*;

/// An interface to universally interact with all allocators with

/// Expectation of an allocator
pub trait Allocator: Clone + Send + Sync {
    type Allocation: Allocation;

    /// Create a new allocation
    fn allocate(
        &mut self,
        name: &str,
        requirements: &vk::MemoryRequirements,
        ty: MemoryType,
    ) -> Result<Self::Allocation>;

    /// Free an allocation
    fn free(&mut self, allocation: Self::Allocation) -> Result<()>;
}

pub trait Allocation: Default {
    /// Get the underlying [`vk::DeviceMemory`]
    fn memory(&self) -> vk::DeviceMemory;

    /// Get the offset of the memory
    fn offset(&self) -> vk::DeviceSize;

    /// Get the raw ptr that underlies the allocation
    fn mapped_ptr(&self) -> Option<NonNull<c_void>>;
    /// Get name of the allocation
    fn name(&self) -> &str;
}
