pub mod allocators;
#[cfg(feature = "bootstrap")]
pub mod bootstrap;
pub mod command;
pub mod context;
pub mod core;
pub mod device;
pub mod error;
pub mod prelude;
pub mod resource;
pub mod sync;
pub mod util;
pub mod wsi;

pub mod descriptor;
mod pipelines;
pub mod shader;
pub mod traits;

pub use error::DagalError;

// Re-exports
pub use ash;
pub use ash_window;
#[cfg(feature = "gpu-allocator")]
pub use gpu_allocator;
pub use raw_window_handle;
#[cfg(feature = "vk-mem-rs")]
pub use vk_mem;
#[cfg(feature = "winit")]
pub use winit;

#[cfg(all(feature = "gpu-allocator", not(feature = "vk-mem-rs")))]
type DEFAULT_ALLOCATOR = allocators::GpuAllocation;
#[cfg(all(feature = "vk-mem-rs", not(feature = "gpu-allocator")))]
type DEFAULT_ALLOCATOR = allocators::GpuAllocation;
