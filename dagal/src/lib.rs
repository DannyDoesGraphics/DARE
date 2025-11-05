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
pub mod pipelines;
pub mod render_graph;
pub mod shader;
pub mod traits;

pub use error::DagalError;
pub type Result<T> = std::result::Result<T, DagalError>;

// Re-exports
#[cfg(feature = "gpu-allocator")]
pub use gpu_allocator;
#[cfg(feature = "vk-mem-rs")]
pub use vk_mem;
#[cfg(feature = "winit")]
pub use winit;
pub use {ash, ash_window, raw_window_handle};

pub type DefaultAllocator = allocators::GPUAllocatorImpl;
