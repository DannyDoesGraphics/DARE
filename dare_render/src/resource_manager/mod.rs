//!
//! # States
//! Describes all states a resources
//! # [`dare_assets::ResidentState::ResidentGPU`]
//! Asset is resident on the GPU, and is ready to be used
//! # [`dare_assets::ResidentState::Unloading`]
//! Asset is being unloaded from the GPU
//! # [`dare_assets::ResidentState::Unloaded`]
//! Asset is no longer resident on the GPU
//! # [`dare_assets::ResidentState::Failed`]
//! Asset failed to load, and must be manually acknowledged by the user
//!
//! # Lifecycle
//! The lifecycle of a resource managed by the resource manager:
//! 1. Resource initiated and inserted with an opaque [`super::ResourceHandle`] handed out
//! 2. Resource is transitioned into `Loading` state and work is started to load the resource onto the GPU
//!  2.1. If the resource fails to load, it is transitioned into `Failed` state and must be manually acknowledged by the user
//! 3. Resource is transitioned into `ResidentGPU` state after being loaded onto the GPU
//! 4. After `n` ticks of no-use for more than the resource's defined TTL, it is transitioned into `Unloading` state
//! 5. After `n` ticks of no-use load or use attempts after more than [`Self::tombstone_ttl`], the resource is unloaded

mod handle;
mod plugin;


pub use plugin::ResourceManagerPlugin;
