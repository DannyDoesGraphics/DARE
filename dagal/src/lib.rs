pub mod allocators;
#[cfg(feature = "bootstrap")]
pub mod bootstrap;
pub mod command;
pub mod core;
pub mod device;
pub mod error;
pub mod prelude;
pub mod resource;
pub mod sync;
pub mod util;
pub mod wsi;

pub mod concurrency;
pub mod descriptor;
mod graph;
pub mod pipelines;
pub mod shader;
pub mod traits;

pub use error::DagalError;

// Re-exports
#[cfg(feature = "gpu-allocator")]
pub use gpu_allocator;
#[cfg(feature = "vk-mem-rs")]
pub use vk_mem;
#[cfg(feature = "winit")]
pub use winit;
pub use {ash, ash_window, raw_window_handle};

#[cfg(all(feature = "gpu-allocator", not(feature = "vk-mem-rs")))]
type DEFAULT_ALLOCATOR = allocators::GpuAllocation;
#[cfg(all(feature = "vk-mem-rs", not(feature = "gpu-allocator")))]
type DEFAULT_ALLOCATOR = allocators::GpuAllocation;
