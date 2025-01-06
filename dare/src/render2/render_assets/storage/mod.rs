use crate::prelude as dare;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dare_containers as containers;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use crossbeam_channel::SendError;
use futures::{FutureExt, TryFutureExt};
use dare_containers::prelude::Slot;
use crate::asset2::prelude::AssetHandle;
use crate::asset2::server::AssetServerDelta;
pub mod handle;
pub mod asset_manager_system;
pub use asset_manager_system::*;
pub use handle::*;

enum InternalLoadedState<T: MetaDataRenderAsset> {
    /// Asset is ready on the GPU to be loaded into
    Readied(T::Loaded),
    /// Asset is entirely loaded
    Loaded(T::Loaded),
}

/// When loading and unloading assets, we need a way to indicate back to the main render thread
/// the assets have been successfully loaded onto the gpu.
///
/// This struct is used to submit to a queue indicating the asset has loaded.
struct RenderAssetStorageLoaded<T: MetaDataRenderAsset> {
    handle: RenderAssetHandle<T>,
    loaded: Result<T::Loaded>,
}

/// Manages linking between the render world <-> asset world of an individual asset type only
///
/// # 2 handles
/// We effectively do have 2 levels of indirection from the asset handle to the resource. This is
/// done to ensure we can separate the lifetimes of render resources from engine lifetimes.
#[derive(becs::Resource)]
pub struct RenderAssetManagerStorage<T: MetaDataRenderAsset> {
    /// Server handle
    asset_server: dare::asset2::server::AssetServer,
    /// Mesh container to tightly pack them
    ///
    /// This is used to help us "tightly" pack, and is used to effectively maintain the bindless
    /// array
    containers: containers::slot_map::SlotMap<AssetHandle<T::Asset>>,
    /// Bindings from asset handles to slots in the slot map
    slot_mappings: HashMap<AssetHandle<T::Asset>, RenderAssetHandle<T>>,
    /// We maintain a queue for dropped proxy handles into the array
    dropped_handles_recv: crossbeam_channel::Receiver<HandleRCDelta<T>>,
    dropped_handles_send: crossbeam_channel::Sender<HandleRCDelta<T>>,
    /// Maintain a list of active handles (ref counting)
    handle_references: HashMap<Slot<AssetHandle<T::Asset>>, u32>,
    /// Links the loaded assets to the asset handle
    internal_loaded: HashMap<RenderAssetHandle<T>, T::Loaded>,
    /// A queue used to handle loaded assets
    asset_loaded_queue_recv: Arc<crossbeam_channel::Receiver<RenderAssetStorageLoaded<T>>>,
    asset_loaded_queue_send: Arc<crossbeam_channel::Sender<RenderAssetStorageLoaded<T>>>,
}

impl<T: MetaDataRenderAsset> RenderAssetManagerStorage<T> {
    pub fn new(asset_server: dare::asset2::server::AssetServer) -> Self {
        let (asset_loaded_queue_send, asset_loaded_queue_recv) = crossbeam_channel::unbounded();
        let (dropped_handles_send, dropped_handles_recv) = crossbeam_channel::unbounded();
        Self {
            asset_server,
            containers: Default::default(),
            slot_mappings: Default::default(),
            dropped_handles_recv,
            dropped_handles_send,
            handle_references: Default::default(),
            internal_loaded: Default::default(),

            asset_loaded_queue_recv: Arc::new(asset_loaded_queue_recv),
            asset_loaded_queue_send: Arc::new(asset_loaded_queue_send),
        }
    }

    /// Process any loaded assets in
    pub fn process_queue(&mut self) {
        // Deal with assets loaded in
        while let Ok(loaded_asset) = self.asset_loaded_queue_recv.try_recv() {
            match loaded_asset.loaded {
                Ok(loaded) => {
                    self.internal_loaded.insert(loaded_asset.handle, loaded);
                }
                Err(e) => {
                    tracing::error!("Failed to load handle {:?}, due to: {:?}", loaded_asset.handle.as_ref(), e)
                }
            }
        }
        // Handle changes to ref counting
        while let Ok(handle) = self.dropped_handles_recv.try_recv() {
            match handle {
                HandleRCDelta::Add(handle) => {
                    if let Some(mut amount) = self.handle_references.get_mut(&handle) {
                        *amount += 1;
                    } else {
                        tracing::warn!("Expected handle, got `None`");
                    }
                }
                HandleRCDelta::Remove(handle) => {
                    // If handle references does not exist, it indicates it mostly has been removed
                    if let Some(mut amount) = self.handle_references.get_mut(&handle) {
                        *amount -= 1;
                        // no refs left, delete
                        if *amount == 0 {
                            // remove whatever is loaded
                            let asset_handle = self.containers.get(handle.as_ref().clone()).cloned();
                            if self.internal_loaded.remove(&handle).is_none() {
                                tracing::warn!("Tried removing handle {:?}, expected loaded, got `None`.", handle.as_ref());
                                // Indicate unloading failed
                                if let Some(asset_handle) = asset_handle {
                                    // Indicate asset was unloaded
                                    unsafe {
                                        self.asset_server.update_state(
                                            &*asset_handle.into_untyped_handle(),
                                            dare::asset2::AssetState::Failed
                                        ).unwrap()
                                    }
                                }
                            } else if let Some(asset_handle) = asset_handle {
                                // Indicate asset was unloaded
                                unsafe {
                                    self.asset_server.update_state(
                                        &*asset_handle.into_untyped_handle(),
                                        dare::asset2::AssetState::Unloaded
                                    ).unwrap()
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Inserts a new asset handle
    pub fn insert(&mut self, handle: AssetHandle<T::Asset>) -> Result<RenderAssetHandle<T>> {
        if self.slot_mappings.contains_key(&handle) {
            return Err(anyhow::Error::msg("Handle already exists"));
        }
        // ensure we only hold weak refs
        let handle = handle.downgrade();
        let slot = self.containers.insert(handle.clone());
        self.handle_references.insert(slot.clone(), 1);
        self.slot_mappings.insert(handle.clone(), RenderAssetHandle::Strong {
            handle: slot.clone(),
            dropped_handles_send: self.dropped_handles_send.clone(),
        });
        {
            let mut hasher = DefaultHasher::new();
            handle.hash(&mut hasher);
            println!("adding {:?} - {:?}", hasher.finish(), handle);
        }
        Ok(RenderAssetHandle::Strong {
            handle: slot,
            dropped_handles_send: self.dropped_handles_send.clone(),
        })
    }

    /// Removes asset handle from render storage, and if exists a loaded asset, it will return it
    pub fn remove(&mut self, handle: RenderAssetHandle<T>) -> Option<T::Loaded> {
        self.containers.remove(handle.as_ref().clone()).unwrap();
        let mut hasher= DefaultHasher::new();
        handle.hash(&mut hasher);
        println!("Removing {:?}", hasher.finish());
        self.handle_references.remove(&handle);
        self.internal_loaded.remove(&handle).map(|loaded| loaded)
    }

    /// Attempts to retrieve the loaded version
    pub fn get_loaded(&self, handle: &RenderAssetHandle<T>) -> Option<&<T as MetaDataRenderAsset>::Loaded> {
        self.internal_loaded.get(handle)
    }

    /// Attempts to retrieve loaded version from asset handle
    pub fn get_loaded_from_asset_handle(&self, asset_handle: &AssetHandle<T::Asset>) -> Option<&<T as MetaDataRenderAsset>::Loaded> {
        self.get_storage_handle(asset_handle).map(|handle| {
            self.get_loaded(&handle)
        })?
    }

    /// Attempts to retrieve the loaded version
    pub fn get_mut_loaded(&mut self, handle: &RenderAssetHandle<T>) -> Option<&mut <T as MetaDataRenderAsset>::Loaded> {
        self.internal_loaded.get_mut(handle)
    }

    /// Get the associated render asset handle for each from an asset handle
    pub fn get_storage_handle(&self, handle: &AssetHandle<T::Asset>) -> Option<RenderAssetHandle<T>> {

        if !self.slot_mappings.contains_key(&handle.clone().downgrade()) {
            for key in self.slot_mappings.keys() {
                let mut hasher = DefaultHasher::new();
                key.hash(&mut hasher);
                println!("keys: {:?}", key);
            }
            let mut hasher = DefaultHasher::new();
            handle.hash(&mut hasher);
            panic!("getting: {:?} - {:?}", handle.clone().downgrade(), handle);
        }
        self.slot_mappings.get(&handle.clone().downgrade()).cloned()
    }

    /// Attempt a load via spawning a dedicated load task
    pub fn load(
        &self,
        handle: &RenderAssetHandle<T>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as dare::asset2::Asset>::Metadata as dare::asset2::loaders::MetaDataLoad>::LoadInfo<'static>,
    ) {
        // Extract `internal_loaded` check into its own scope
        if self.internal_loaded.get(handle).is_some() {
            // Already loaded, do not load again
            return;
        }

        // Extract `containers.get` result into a local variable
        let asset_handle = match self.containers.get(handle.as_ref().clone()) {
            Some(asset_handle) => asset_handle.clone(), // Clone now to avoid borrow issues
            None => return,
        };

        // Extract `asset_server.get_metadata` result
        let metadata = match self
            .asset_server
            .get_metadata_untyped::<T::Asset>(&asset_handle.clone().into_untyped_handle())
        {
            Some(metadata) => metadata,
            None => return,
        };

        // Clone variables used in the async block
        let loaded_send = self.asset_loaded_queue_send.clone();
        let handle = handle.clone();
        let asset_server = self.asset_server.clone();

        // Spawn the async task
        tokio::task::spawn(async move {
            let loaded = T::load_asset(metadata, prepare_info, load_info).await;
            unsafe {
                asset_server
                    .update_state(&*asset_handle.clone().into_untyped_handle(), dare::asset2::AssetState::Loaded)
                    .unwrap();
            }

            // Handle the result of the loading process
            match loaded {
                Ok(loaded) => {
                    if let Err(e) = loaded_send.send(RenderAssetStorageLoaded {
                        handle,
                        loaded: Ok(loaded),
                    }) {
                        tracing::error!("Failed to send finished asset: {e}");
                    }
                }
                Err(e) => {
                    if let Err(e) = loaded_send.send(RenderAssetStorageLoaded {
                        handle,
                        loaded: Err(e),
                    }) {
                        tracing::error!("Failed to send failed asset: {e}");
                    }
                }
            }
        });
    }

    pub fn asset_server(&self) -> dare::asset2::server::AssetServer {
        self.asset_server.clone()
    }
}

impl RenderAssetManagerStorage<dare::render::render_assets::components::buffer::RenderBuffer<GPUAllocatorImpl>> {
    pub fn get_bda(&self, handle: &RenderAssetHandle<dare::render::render_assets::components::RenderBuffer<GPUAllocatorImpl>>) -> Option<vk::DeviceAddress> {
        self.internal_loaded.get(handle).map(|slot| {
            slot.buffer.address()
        })
    }

    pub fn get_bda_from_asset_handle(&self, handle: &AssetHandle<
        dare::asset2::assets::Buffer
    >) -> Option<vk::DeviceAddress> {
        self.get_loaded_from_asset_handle(handle).map(|buffer| {
            buffer.address()
        })
    }
}