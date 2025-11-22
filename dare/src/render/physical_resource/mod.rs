use crate::asset::prelude::AssetHandle;
use crate::asset::traits::Asset;
use bevy_ecs::prelude::*;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use futures::TryFutureExt;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use traits::MetaDataRenderAsset;

pub mod gpu_stream;
pub use gpu_stream::*;
pub mod handle;
pub mod render_buffer;
pub mod render_image;
pub mod surface_soa;
/// Handles render components
pub mod traits;

use crate::asset::loaders::MetaDataLoad;
use crate::asset::prelude as asset;
use crate::asset::server::AssetServer;
pub use handle::*;
pub use render_buffer::*;
pub use render_image::*;

/// Internalize deletion entry
#[derive(Default)]
struct DeletionSlot {
    lifetime: u32,
    current: u32,
    virtual_resource: Option<VirtualResource>,
}
impl DeletionSlot {
    /// Reset current lifetime
    pub fn reset(&mut self) {
        self.current = self.lifetime
    }
}

pub(crate) enum PhysicalState<T> {
    /// Represents a **temporary** unused state
    Empty,
    /// Load lock to prevent double loads
    Loading,
    /// Existing
    Some(T),
}
impl<T> PhysicalState<T> {
    pub fn is_some(&self) -> bool {
        matches!(self, PhysicalState::Some(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, PhysicalState::Empty)
    }

    #[allow(dead_code)]
    pub fn map<F: FnOnce(&T) -> A, A>(&self, f: F) -> Option<A> {
        match self {
            PhysicalState::Loading | PhysicalState::Empty => None,
            PhysicalState::Some(t) => Some(f(t)),
        }
    }

    pub fn replace(&mut self, val: T) -> Option<T> {
        match self {
            PhysicalState::Loading | PhysicalState::Empty => {
                *self = PhysicalState::Some(val);
                None
            }
            PhysicalState::Some(t) => Some(std::mem::replace(t, val)),
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            PhysicalState::Some(t) => Some(t),
            _ => None,
        }
    }
}

/// Storage for physical resources and used to allocate out [`VirtualResource`]
///
/// # Error handling
/// When loading, if an asset fails to properly load, we will not record that it failed.
#[derive(Resource)]
pub(crate) struct PhysicalResourceStorage<T: MetaDataRenderAsset> {
    pub asset_server: AssetServer,
    pub(crate) slot: containers::SlotMap<PhysicalState<T::Loaded>, VirtualResource>,
    /// Map asset handles to virtual resource handles
    asset_mapping: HashMap<AssetHandle<T::Asset>, VirtualResource>,
    asset_mapping_reverse: HashMap<VirtualResource, AssetHandle<T::Asset>>,
    /// for asset loading
    loaded_send: crossbeam_channel::Sender<(VirtualResource, T::Loaded)>,
    loaded_recv: crossbeam_channel::Receiver<(VirtualResource, T::Loaded)>,
    /// for handle drops
    drop_send: crossbeam_channel::Sender<VirtualResource>,
    drop_recv: crossbeam_channel::Receiver<VirtualResource>,
    /// We can also perform deferred deletion strategies
    /// (current_lifetime, old_lifetime, reference)
    deferred_deletion: HashMap<VirtualResource, DeletionSlot>,
}

impl<T: MetaDataRenderAsset> PhysicalResourceStorage<T> {
    pub fn new(asset_server: AssetServer) -> Self {
        let (loaded_send, loaded_recv) = crossbeam_channel::unbounded();
        let (drop_send, drop_recv) = crossbeam_channel::unbounded();
        Self {
            asset_server,
            slot: Default::default(),
            asset_mapping: Default::default(),
            asset_mapping_reverse: Default::default(),
            loaded_send,
            loaded_recv,
            drop_send,
            drop_recv,
            deferred_deletion: Default::default(),
        }
    }

    /// Allocate a virtual resource handle that is in the process of loading
    #[allow(dead_code)]
    pub fn get_virtual_handle(&mut self, lifetime: Option<u32>) -> VirtualResource {
        let mut slot = self.slot.insert(PhysicalState::Loading);

        // Set up drop semantics if this is not a deferred deletion resource
        // Deferred deletion resources are managed by lifetime counters, not drop semantics
        if lifetime.is_none() {
            slot.set_drop_semantics(Some(self.drop_send.clone()));
        }

        match lifetime {
            None => slot,
            Some(lifetime) => {
                let deletion_slot = self.deferred_deletion.entry(slot.clone()).or_default();
                deletion_slot.lifetime = lifetime;
                slot
            }
        }
    }

    /// Insert a physical resource to back a virtual resource
    #[allow(dead_code)]
    pub fn alias(
        &mut self,
        virtual_resource: &VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(virtual_resource.clone())
            .and_then(|option| option.replace(physical_resource))
    }

    /// Alias an asset handle to a virtual resource
    #[allow(dead_code)]
    pub fn asset_alias(
        &mut self,
        virtual_resource: &VirtualResource,
        handle: AssetHandle<T::Asset>,
    ) -> Option<VirtualResource> {
        self.asset_mapping_reverse
            .insert(virtual_resource.downgrade(), handle.clone().downgrade());
        self.asset_mapping
            .insert(handle.downgrade(), virtual_resource.downgrade())
    }

    /// Insert a deferred resource
    #[allow(dead_code)]
    pub fn alias_deferred(
        &mut self,
        virtual_resource: VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(virtual_resource.clone())
            .and_then(|option| {
                // reset lifetime
                if let Some(deferred) = self.deferred_deletion.get_mut(&virtual_resource) {
                    deferred.reset()
                }
                option.replace(physical_resource)
            })
    }
    /// Insert a deferred physical resource back to a new virtual resource
    #[allow(dead_code)]
    pub fn insert_deferred(
        &mut self,
        lifetime: Option<u32>,
        physical_resource: T::Loaded,
    ) -> VirtualResource {
        let virtual_handle = self.get_virtual_handle(lifetime);
        if lifetime.map(|v| v > 0).unwrap_or(false) {
            let deletion = self.deferred_deletion.get_mut(&virtual_handle).unwrap(); // unwrap should be *fine* here, since [`Self::get_virtual_handle`] properly sets up the deletion entry.
            deletion.reset();
            deletion.virtual_resource.replace(virtual_handle.clone());
        }
        self.alias(&virtual_handle, physical_resource); // we don't need to do deferred for now...
        virtual_handle
    }

    /// Acquire a channel to notify physical resource storage of successful asset loading
    #[allow(dead_code)]
    pub fn asset_loaded_queue(&self) -> crossbeam_channel::Sender<(VirtualResource, T::Loaded)> {
        self.loaded_send.clone()
    }

    /// Attempt to resolve a virtual resource
    pub fn resolve(&mut self, virtual_resource: &VirtualResource) -> Option<&T::Loaded> {
        self.slot
            .get(virtual_resource.clone())
            .and_then(|option| match option.as_ref() {
                None => None,
                Some(r) => {
                    // keep alive
                    if let Some(deferred) = self.deferred_deletion.get_mut(virtual_resource) {
                        deferred.reset()
                    }
                    Some(r)
                }
            })
    }

    /// Resolve using an asset instead of virtual resource
    pub fn resolve_asset(&mut self, asset_handle: &AssetHandle<T::Asset>) -> Option<&T::Loaded> {
        match self.asset_mapping.get(asset_handle).cloned() {
            None => None,
            Some(v) => self.resolve(&v),
        }
    }

    /// Resolve for a virtual resource handle that is mapped to an [`AssetHandle`]
    pub fn resolve_virtual_resource(
        &self,
        virtual_resource: &AssetHandle<T::Asset>,
    ) -> Option<&VirtualResource> {
        self.asset_mapping.get(virtual_resource)
    }

    /// Process loaded assets
    pub fn update(&mut self) {
        // handle inserts
        while let Ok((virtual_resource, physical_resource)) = self.loaded_recv.try_recv() {
            // find each loaded, update virtual resource to reflect new asset
            let old = self.slot.get_mut(virtual_resource.clone()).unwrap();
            if !old.is_some() {
                old.replace(physical_resource);
            } else {
                tracing::error!("Attempted to replace an already existing resource");
            }
        }
        // decrement lifetimes
        for deferred in self
            .deferred_deletion
            .values_mut()
            .filter(|v| v.lifetime != 0 && v.virtual_resource.is_some())
        {
            deferred.current = deferred.current.saturating_sub(1);
            if deferred.current == 0 {
                // lifetime has reached zero, get rid of the call strong handle we have on it
                deferred.virtual_resource.take();
            }
        }
        // remove dropped
        while let Ok(virtual_resource) = self.drop_recv.try_recv() {
            if let Ok(t) = self.slot.remove(virtual_resource.clone()) {
                self.asset_mapping_reverse
                    .remove(&virtual_resource)
                    .map(|vr| self.asset_mapping.remove(&vr));
                drop(t);
            }
        }
    }

    /// If an asset has an associated asset handle already, we can use that to automatically load it
    /// from a different thread
    pub fn load_asset_handle(
        &mut self,
        asset_handle: &AssetHandle<T::Asset>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static>,
    ) {
        let finished_queue = self.loaded_send.clone();
        let virtual_handle = self.asset_mapping.get(asset_handle).unwrap();
        let is_unloaded = self
            .slot
            .get(virtual_handle.clone())
            .map(|state| state.is_none())
            .unwrap_or(false);
        if is_unloaded {
            let virtual_handle = virtual_handle.clone();
            // get deferred deletion and reset
            if let Some(deferred_deletion) = self.deferred_deletion.get_mut(&virtual_handle) {
                deferred_deletion.reset();
            }
            // apply lock
            if let Some(slot) = self.slot.get_mut(virtual_handle.clone()) {
                *slot = PhysicalState::Loading;
            }
            if let Some(metadata) = self.asset_server.get_metadata(&asset_handle) {
                tokio::task::spawn(async move {
                    T::load_asset(metadata, prepare_info, load_info)
                        .and_then(|v| async move {
                            if finished_queue.send((virtual_handle.clone(), v)).is_err() {
                                tracing::error!(
                                    "Physical resource storage failed to send loaded resource"
                                );
                            };
                            Ok(())
                        })
                        .await
                });
            }
        }
    }

    /// Similarly to `load_asset_handle` however, we will insert the resource and perform a load if it does not exist
    pub fn load_or_create_asset_handle(
        &mut self,
        asset_handle: AssetHandle<T::Asset>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static>,
        lifetime: u32,
    ) {
        let asset_handle = asset_handle.downgrade();
        if self
            .asset_mapping
            .get(&asset_handle)
            .map(|vr| vr.upgrade().is_none())
            .unwrap_or(true)
        {
            let virtual_resource = {
                let mut slot = self.slot.insert(PhysicalState::Empty);
                slot.set_drop_semantics(Some(self.drop_send.clone()));

                let deletion_slot = self.deferred_deletion.entry(slot.downgrade()).or_default();
                deletion_slot.virtual_resource = Some(slot.clone());
                deletion_slot.lifetime = lifetime;
                slot
            }
            .downgrade();
            // apply mapping
            self.asset_mapping_reverse
                .insert(virtual_resource.clone(), asset_handle.clone());
            self.asset_mapping
                .insert(asset_handle.clone(), virtual_resource);
        }
        self.load_asset_handle(&asset_handle, prepare_info, load_info);
    }

    /// Given an asset handle, keep it's respective virtual resource alive
    pub fn keep_alive_from_asset_handle(
        &mut self,
        asset_handle: &AssetHandle<T::Asset>,
    ) -> Option<VirtualResource> {
        if let Some(vr) = self.asset_mapping.get(asset_handle) {
            // keep alive
            if let Some(deferred) = self.deferred_deletion.get_mut(vr) {
                deferred.reset();
            }
            Some(vr.clone())
        } else {
            None
        }
    }
}

impl<A: Allocator + 'static> PhysicalResourceStorage<RenderBuffer<A>> {
    pub fn get_bda(
        &mut self,
        asset_handle: &AssetHandle<asset::assets::Buffer>,
    ) -> Option<vk::DeviceAddress> {
        if let Some(vr) = self
            .asset_mapping
            .get(asset_handle)
            .and_then(|vr| vr.upgrade())
        {
            self.slot.get(vr.clone())?.as_ref().map(|buf| {
                if let Some(deferred) = self.deferred_deletion.get_mut(&vr) {
                    deferred.reset()
                }
                buf.address()
            })
        } else {
            None
        }
    }
}

/// Similar to a [`PhysicalResourceStorage`] however with the difference that we maintain hash
/// uniqueness per asset create information.
///
/// This is primarily useful if we have certain objects such as Samplers which are universal across
/// images
pub struct PhysicalResourceHashMap<
    T: MetaDataRenderAsset<Loaded = L, Asset = A>,
    A: Hash + PartialEq + Eq + Clone,
    L: Send + Resource,
> {
    resource: PhysicalResourceStorage<T>,
    map: HashMap<A, VirtualResource>,
}

/// Optimized direct resource storage with minimal indirection
///
/// This replaces the complex virtual resource system with direct mapping
/// for better performance in hot paths.
#[derive(Resource)]
pub struct DirectResourceStorage<T: MetaDataRenderAsset> {
    /// Direct mapping from asset handle to physical resource
    resources: dare_containers::dashmap::DashMap<AssetHandle<T::Asset>, Arc<T::Loaded>>,
    /// Usage tracking for cleanup - only track recently used resources
    usage_tracker: dare_containers::dashmap::DashMap<AssetHandle<T::Asset>, Instant>,
    /// Generation counter for cache invalidation
    generation: AtomicU64,
    /// Asset server reference for metadata
    asset_server: AssetServer,
    /// Cleanup threshold - resources unused for this long get cleaned up
    cleanup_threshold: Duration,
}

impl<T: MetaDataRenderAsset> DirectResourceStorage<T> {
    pub fn new(asset_server: AssetServer) -> Self {
        Self {
            resources: dare_containers::dashmap::DashMap::new(),
            usage_tracker: dare_containers::dashmap::DashMap::new(),
            generation: AtomicU64::new(0),
            asset_server,
            cleanup_threshold: Duration::from_secs(30), // 30 seconds cleanup threshold
        }
    }

    /// Fast path resource resolution with minimal overhead
    pub fn resolve(&self, asset_handle: &AssetHandle<T::Asset>) -> Option<Arc<T::Loaded>> {
        if let Some(resource) = self.resources.get(asset_handle) {
            // Update usage time for this resource
            self.usage_tracker
                .insert(asset_handle.clone(), Instant::now());
            Some(resource.clone())
        } else {
            None
        }
    }

    /// Batch resolve multiple resources - more efficient than individual calls
    pub fn resolve_batch(
        &self,
        asset_handles: &[AssetHandle<T::Asset>],
    ) -> Vec<Option<Arc<T::Loaded>>> {
        let now = Instant::now();
        asset_handles
            .iter()
            .map(|handle| {
                if let Some(resource) = self.resources.get(handle) {
                    self.usage_tracker.insert(handle.clone(), now);
                    Some(resource.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Insert a resource directly (for immediate loading)
    pub fn insert(&self, asset_handle: AssetHandle<T::Asset>, resource: T::Loaded) {
        let arc_resource = Arc::new(resource);
        self.resources.insert(asset_handle.clone(), arc_resource);
        self.usage_tracker.insert(asset_handle, Instant::now());
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Efficient cleanup of unused resources
    pub fn cleanup_unused(&self) {
        let now = Instant::now();
        let threshold = self.cleanup_threshold;

        // Collect handles to remove
        let mut to_remove = Vec::new();
        for entry in self.usage_tracker.iter() {
            if now.duration_since(*entry.value()) > threshold {
                to_remove.push(entry.key().clone());
            }
        }

        // Remove unused resources
        let mut removed_count = 0;
        for handle in to_remove {
            if self.resources.remove(&handle).is_some() {
                self.usage_tracker.remove(&handle);
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            self.generation.fetch_add(1, Ordering::Relaxed);
            tracing::debug!("Cleaned up {} unused resources", removed_count);
        }
    }

    /// Force cleanup of specific resource
    pub fn remove(&self, asset_handle: &AssetHandle<T::Asset>) -> bool {
        let removed = self.resources.remove(asset_handle).is_some();
        self.usage_tracker.remove(asset_handle);
        if removed {
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
        removed
    }

    /// Get current generation for cache invalidation
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Check if a resource exists without updating usage
    pub fn contains(&self, asset_handle: &AssetHandle<T::Asset>) -> bool {
        self.resources.contains_key(asset_handle)
    }

    /// Get resource count for monitoring
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Get metadata for an asset
    pub fn get_metadata(
        &self,
        asset_handle: &AssetHandle<T::Asset>,
    ) -> Option<<T::Asset as Asset>::Metadata> {
        self.asset_server.get_metadata(asset_handle)
    }
}

/// Cache-friendly resource resolver for hot paths
///
/// This provides a small cache for the most frequently accessed resources
/// to minimize hash map lookups in critical rendering loops.
pub struct CachedResourceResolver<T: MetaDataRenderAsset> {
    /// The underlying storage
    storage: Arc<DirectResourceStorage<T>>,
    /// Simple cache entry
    hot_cache: std::sync::Mutex<Vec<(AssetHandle<T::Asset>, Arc<T::Loaded>, u64)>>,
    /// Cache generation for invalidation
    cache_generation: AtomicU64,
}

impl<T: MetaDataRenderAsset> CachedResourceResolver<T> {
    pub fn new(storage: Arc<DirectResourceStorage<T>>) -> Self {
        Self {
            storage,
            hot_cache: std::sync::Mutex::new(Vec::with_capacity(16)),
            cache_generation: AtomicU64::new(0),
        }
    }

    /// Ultra-fast resource resolution with caching
    pub fn resolve_cached(&self, asset_handle: &AssetHandle<T::Asset>) -> Option<Arc<T::Loaded>> {
        let current_generation = self.storage.generation();

        // Check cache first
        if let Ok(cache) = self.hot_cache.try_lock() {
            for (cached_handle, cached_resource, cached_generation) in cache.iter() {
                if cached_generation == &current_generation && cached_handle == asset_handle {
                    return Some(cached_resource.clone());
                }
            }
        }

        // Cache miss - go to storage
        if let Some(resource) = self.storage.resolve(asset_handle) {
            // Try to update cache
            if let Ok(mut cache) = self.hot_cache.try_lock() {
                // Simple replacement strategy - if cache is full, remove oldest
                if cache.len() >= 16 {
                    cache.remove(0);
                }
                cache.push((asset_handle.clone(), resource.clone(), current_generation));
            }

            Some(resource)
        } else {
            None
        }
    }

    /// Invalidate cache when storage changes
    pub fn invalidate_cache(&self) {
        self.cache_generation.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut cache) = self.hot_cache.try_lock() {
            cache.clear();
        }
    }
}

impl<A: Allocator + 'static> DirectResourceStorage<RenderBuffer<A>> {
    /// Fast path for getting buffer device address
    pub fn get_bda(
        &self,
        asset_handle: &AssetHandle<asset::assets::Buffer>,
    ) -> Option<vk::DeviceAddress> {
        self.resolve(asset_handle).map(|buffer| buffer.address())
    }

    /// Batch get multiple BDAs for efficiency
    pub fn get_bdas(
        &self,
        asset_handles: &[AssetHandle<asset::assets::Buffer>],
    ) -> Vec<Option<vk::DeviceAddress>> {
        asset_handles
            .iter()
            .map(|handle| self.get_bda(handle))
            .collect()
    }
}

impl<
    T: MetaDataRenderAsset<Loaded = L, Asset = A>,
    A: Hash + PartialEq + Eq + Clone,
    L: Send + Resource,
> PhysicalResourceHashMap<T, A, L>
{
    /// Create a new hashmap
    pub fn new(asset_server: AssetServer) -> Self {
        Self {
            resource: PhysicalResourceStorage::new(asset_server),
            map: HashMap::new(),
        }
    }

    /// Attempts to retrieve using create info, if not, force a load to occur
    pub fn retrieve(&mut self, ci: A) -> Option<&T::Loaded> {
        // do a check if the virtual resource exists
        let vr_option = self.map.get(&ci).cloned();
        let is_vr_option = vr_option.is_some();
        if let Some(vr) = vr_option {
            // mapping found, attempt to retrieve it
            if self.resource.resolve(&vr).is_some() {
                return self.resource.resolve(&vr);
            }
            // if not, go try to retrieve again
        }
        // make a new resource, since we cannot resolve it
        let virtual_resource = self.resource.get_virtual_handle(Some(0));
        // if mapping doesn't exist yet, map it
        if !is_vr_option {
            self.map.insert(ci, virtual_resource.clone());
        }
        //self.resource.alias(&virtual_resource, loaded);
        self.resource.resolve(&virtual_resource)
    }

    /// Retrieve the underlying [`PhysicalResourceStorage`]
    pub fn physical_resource_storage(&self) -> &PhysicalResourceStorage<T> {
        &self.resource
    }
}
