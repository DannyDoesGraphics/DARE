use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use dare_containers as containers;
use crate::prelude as dare;
use anyhow::Result;


#[derive(Debug)]
pub struct RenderAssetMangerInfo<T: dare::render::render_assets::traits::MetaDataRenderAsset> {
}

enum InternalLoadedState<T: dare::render::render_assets::traits::MetaDataRenderAsset> {
    /// Asset is ready on the GPU to be loaded into
    Readied(T::Loaded),
    /// Asset is entirely loaded
    Loaded(T::Loaded),
}

/// Manages linking between the render world <-> asset world of an individual asset type only
pub struct RenderAssetManagerStorage<T: dare::render::render_assets::traits::MetaDataRenderAsset> {
    /// Internal hash of all currently used slot maps
    hash: u64,
    /// Mesh container to tightly pack them
    ///
    /// This is used to help us "tightly" pack
    containers: containers::slot_map::SlotMap<dare::asset2::AssetHandle<T::Asset>>,
    /// Linking asset -> internal slot
    internal_linking: HashMap<dare::asset2::AssetHandle<T::Asset>, containers::slot::Slot<dare::asset2::AssetHandle<T::Asset>>>,
    /// Links the loaded assets to the asset handle
    internal_loaded: HashMap<dare::asset2::AssetHandle<T::Asset>, T::Loaded>,
    /// Effectively functions as a queue, but handles asset loading
    asset_loaded_recv: crossbeam_channel::Receiver<InternalLoadedState<T>>,
    asset_loaded_send: crossbeam_channel::Sender<InternalLoadedState<T>>,
}

impl<T: dare::render::render_assets::traits::MetaDataRenderAsset> RenderAssetManagerStorage<T> {
    pub fn new() -> Self {
        let (asset_loaded_send, asset_loaded_recv) = crossbeam_channel::unbounded();
        Self {
            hash: 0,
            containers: Default::default(),
            internal_linking: Default::default(),
            internal_loaded: Default::default(),
            asset_loaded_recv,
            asset_loaded_send,
        }
    }

    /// Inserts a new handle
    pub fn insert(&mut self, handle: dare::asset2::AssetHandle<T::Asset>) -> anyhow::Result<()> {
        // ensure we only hold weak
        let handle = handle.downgrade();
        let slot = self.containers.insert(handle.clone());
        self.internal_linking.insert(handle.clone(), slot);
        let mut hash = std::hash::DefaultHasher::default();
        for (e, _) in self.containers.iter() {
            e.hash(&mut hash);
        }
        self.hash = hash.finish();

        Ok(())
    }

    /// Removes from render storage, and if exists a loaded asset, it will return it
    pub fn remove(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<T::Loaded> {
        self.internal_linking.remove(handle).map(|slot| {
            self.containers.remove(slot)
        });
        self.internal_loaded.remove(handle)
    }

    /// Attempts to retrieve the loaded version
    pub fn get_loaded(&self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<&T::Loaded> {
        self.internal_loaded.get(handle)
    }

    /// Attempts to retrieve the loaded version
    pub fn get_mut_loaded(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<&mut T::Loaded> {
        if self.internal_linking.get(&handle).is_some() {
            self.internal_loaded.get_mut(handle)
        } else {
            None
        }
    }

    /// If the internal asset is loaded, attempts to fetch
    pub fn get_mut_loaded_or_load(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>, load_info: T::PrepareInfo) -> Option<Result<&mut T::Loaded>> {
        if self.internal_linking.get(&handle).is_some() {
            Some(self.internal_loaded.get_mut(handle).map_or({
                                                             self.internal_linking.insert(handle.clone(), T::load_asset(load_info, prepare_info, load_info));
                                                             }, Ok)
        } else {
            None
        }
    }

}