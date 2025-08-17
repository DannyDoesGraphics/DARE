//! Defines the resource management system to load in/out resources on the GPU
//!
//! # Resource lifecycle
//! Resources are first registered with the resource manager, which keeps track of all
//! resources through sending their metadata. Each metadata must be unique and same
//! metadata hashes will be assumed to reference the same resource.
//!
//! ## Loading
//! Resources are first set to be non-cpu loaded, implying they do not exist on the cpu.
//! When a load is requested a transient CPU form is created.
//!
//! ### Streaming
//! If a resource supports [`ResourceMetadata::stream`], a transient stream struct will
//! be created responsible for streaming in the resource's data.
//!
//! If a resource however does not support [`ResourceMetadata::stream`], it will be loaded in
//! using a single method on [`Resource::Metadata`] through [`ResourceMetadata::load`].
//!
//! ## GPU
//! If a resource has the trait [`ResourceGPU`], and has been requested to be loaded onto
//! the GPU, an additional transient GPU struct will be created.

pub mod client;
pub mod handle;
pub mod server;
pub mod traits;
pub use traits::*;
pub mod resources;
pub mod streams;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ResourceLocation {
    URL(String),
    FilePath(std::path::PathBuf),
    Memory(std::sync::Arc<[u8]>),
}
