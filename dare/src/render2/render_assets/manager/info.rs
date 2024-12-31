use crate::prelude as dare;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use anyhow::Result;
use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dare_containers as containers;
use dare_containers::dashmap::mapref::one::{Ref, RefMut};
use dare_containers::dashmap::DashMap;
use std::hash::Hash;

enum InternalLoadedState<T: MetaDataRenderAsset> {
    /// Asset is ready on the GPU to be loaded into
    Readied(T::Loaded),
    /// Asset is entirely loaded
    Loaded(T::Loaded),
}

/// Manages linking between the render world <-> asset world of an individual asset type only
pub struct RenderAssetManagerStorage<T: MetaDataRenderAsset> {
    /// Server handle
    asset_server: dare::asset2::server::AssetServer,
    /// Mesh container to tightly pack them
    ///
    /// This is used to help us "tightly" pack
    containers: containers::slot_map::SlotMap<dare::asset2::AssetHandle<T::Asset>>,
    /// Linking asset -> internal slot
    internal_linking: DashMap<dare::asset2::AssetHandle<T::Asset>, containers::slot::Slot<dare::asset2::AssetHandle<T::Asset>>>,
    /// Links the loaded assets to the asset handle
    internal_loaded: DashMap<dare::asset2::AssetHandle<T::Asset>, T::Loaded>,
}

impl<T: MetaDataRenderAsset> RenderAssetManagerStorage<T> {
    pub fn new(asset_server: dare::asset2::server::AssetServer) -> Self {
        Self {
            asset_server,
            containers: Default::default(),
            internal_linking: Default::default(),
            internal_loaded: Default::default(),
        }
    }

    /// Inserts a new handle
    pub fn insert(&mut self, handle: dare::asset2::AssetHandle<T::Asset>) -> Result<()> {
        // ensure we only hold weak
        let handle = handle.downgrade();
        let slot = self.containers.insert(handle.clone());
        self.internal_linking.insert(handle.clone(), slot);
        Ok(())
    }

    /// Removes from render storage, and if exists a loaded asset, it will return it
    pub fn remove(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<T::Loaded> {
        self.internal_linking.remove(handle).map(|(asset, slot)| {
            self.containers.remove(slot)
        });
        self.internal_loaded.remove(handle).map(|(asset, loaded)| loaded)
    }

    /// Attempts to retrieve the loaded version
    pub fn get_loaded(&self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<Ref<'_, dare::asset2::AssetHandle<<T as MetaDataRenderAsset>::Asset>, <T as MetaDataRenderAsset>::Loaded>> {
        self.internal_loaded.get(handle)
    }

    /// Attempts to retrieve the loaded version
    pub fn get_mut_loaded(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>) -> Option<RefMut<'_, dare::asset2::AssetHandle<<T as MetaDataRenderAsset>::Asset>, <T as MetaDataRenderAsset>::Loaded>> {
        if self.internal_linking.get(&handle).is_some() {
            self.internal_loaded.get_mut(handle)
        } else {
            None
        }
    }

    /// Retrieve using the asset's handle, if it exists, it will grab the loaded, if not
    /// it will try to load the asset in new
    pub async fn get_mut_loaded_or_load(&mut self, handle: &dare::asset2::AssetHandle<T::Asset>, prepare_info: T::PrepareInfo, load_info: <<T::Asset as dare::asset2::Asset>::Metadata as dare::asset2::loaders::MetaDataLoad>::LoadInfo<'_>) -> Option<RefMut<'_, dare::asset2::AssetHandle<<T as MetaDataRenderAsset>::Asset>, <T as MetaDataRenderAsset>::Loaded>> {
        if self.internal_loaded.get(handle).is_some() {
            self.internal_loaded.get_mut(handle)
        } else if let Some(metadata) = self.asset_server.get_metadata::<T::Asset>(&handle.clone().into_untyped_handle()) {
            self.internal_loaded.insert(
                handle.clone(),
                T::load_asset(
                    metadata,
                    prepare_info,
                    load_info
                ).await.ok()?
            );
            self.internal_loaded.get_mut(handle)
        } else {
            // Asset doesn't exist in the asset server???
            None
        }
    }
}

impl RenderAssetManagerStorage<dare::render::render_assets::components::buffer::RenderBuffer<GPUAllocatorImpl>> {
    pub fn get_bda(&self, handle: &dare::asset2::AssetHandle<dare::asset2::assets::Buffer>) -> Option<vk::DeviceAddress> {
        self.internal_loaded.get(handle).map(|slot| {
            slot.buffer.address()
        })
    }
}