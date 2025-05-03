use crate::prelude as dare;
use crate::render2::physical_resource;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dare::asset2 as asset;
use std::collections::HashMap;

/// Stores render assets densely packed
pub struct HashRenderAssetStorage<T: super::traits::MetaDataRenderAsset> {
    pub assets: HashMap<asset::AssetIdUntyped, Option<T::Loaded>>,
}

impl<T: super::traits::MetaDataRenderAsset> Default for HashRenderAssetStorage<T> {
    fn default() -> Self {
        Self {
            assets: HashMap::default(),
        }
    }
}

pub enum RenderAssetDelta<T: super::traits::MetaDataRenderAsset> {
    Add {
        asset_id: asset::AssetId<T::Asset>,
        render_asset: T::Loaded,
    },
    Remove(asset::AssetId<T::Asset>),
}

#[derive(Clone)]
pub struct RenderAssets<T: super::traits::MetaDataRenderAsset> {
    send_deltas: crossbeam_channel::Sender<RenderAssetDelta<T>>,
}
impl<T: super::traits::MetaDataRenderAsset> RenderAssets<T> {
    pub fn insert(&self, asset_id: asset::AssetId<T::Asset>, render_asset: T::Loaded) {
        self.send_deltas
            .send(RenderAssetDelta::Add {
                asset_id,
                render_asset,
            })
            .unwrap()
    }

    pub fn remove(&self, handle: asset::AssetId<T::Asset>) {
        self.send_deltas
            .send(RenderAssetDelta::Remove(handle))
            .unwrap();
    }
}

#[derive(becs::Resource)]
pub struct RenderAssetsStorage<T: super::traits::MetaDataRenderAsset> {
    pub dense_render_assets: HashRenderAssetStorage<T>,
    recv_deltas: crossbeam_channel::Receiver<RenderAssetDelta<T>>,
    send_deltas: crossbeam_channel::Sender<RenderAssetDelta<T>>,
}
impl<T: super::traits::MetaDataRenderAsset> Default for RenderAssetsStorage<T> {
    fn default() -> Self {
        let (send_deltas, recv_deltas) = crossbeam_channel::unbounded();
        Self {
            dense_render_assets: HashRenderAssetStorage::default(),
            recv_deltas,
            send_deltas,
        }
    }
}
impl<T: super::traits::MetaDataRenderAsset> RenderAssetsStorage<T> {
    /// Processes delta queue
    pub fn process(&mut self) {
        while let Ok(delta) = self.recv_deltas.try_recv() {
            match delta {
                RenderAssetDelta::Add {
                    asset_id: handle,
                    render_asset,
                } => {
                    let displaced = self
                        .dense_render_assets
                        .assets
                        .insert(handle.as_untyped_id(), Some(render_asset));
                    if displaced.is_some() {
                        panic!("We displaced?")
                    }
                }
                RenderAssetDelta::Remove(handle) => {
                    self.dense_render_assets
                        .assets
                        .insert(handle.as_untyped_id(), None);
                }
            }
        }
    }

    pub fn get(&self, handle: &asset::AssetId<T::Asset>) -> Option<&T::Loaded> {
        self.dense_render_assets
            .assets
            .get(&handle.as_untyped_id())
            .map(|v| v.as_ref().map(|v| v))
            .flatten()
    }

    /// Acquire a manager
    pub fn server(&self) -> RenderAssets<T> {
        RenderAssets {
            send_deltas: self.send_deltas.clone(),
        }
    }
}
impl RenderAssetsStorage<physical_resource::RenderBuffer<GPUAllocatorImpl>> {
    /// Get bda
    pub fn get_bda(
        &self,
        asset_id: &asset::AssetId<dare::asset2::assets::Buffer>,
    ) -> Option<vk::DeviceSize> {
        self.dense_render_assets
            .assets
            .get(&asset_id.as_untyped_id())
            .map(|v| v.as_ref().map(|render_buffer| render_buffer.address()))
            .flatten()
    }
}
