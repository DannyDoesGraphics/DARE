pub mod asset_info;
pub mod deltas;

use super::prelude as asset;
use bevy_ecs::prelude::*;
use dare_containers::dashmap::try_result::TryResult;
pub use deltas::AssetServerDelta;
use std::any::TypeId;
use std::sync::Arc;

/// Asset server (engine side)
#[derive(Debug)]
pub struct AssetServerInner {
    delta_send: crossbeam_channel::Sender<AssetServerDelta>,
    delta_recv: crossbeam_channel::Receiver<AssetServerDelta>,
    /// Cloned and handed out to [`asset::StrongAssetHandleUntyped`] to be sent upon struct being
    /// dropped and adds it to the queue to set the asset state to be unloaded.
    drop_send: crossbeam_channel::Sender<asset::AssetIdUntyped>,
    /// Receives all drop requests
    drop_recv: crossbeam_channel::Receiver<asset::AssetIdUntyped>,
}

impl Default for AssetServerInner {
    fn default() -> Self {
        let (delta_send, delta_recv) = crossbeam_channel::unbounded();
        let (drop_send, drop_recv) = crossbeam_channel::unbounded();
        Self {
            delta_send,
            delta_recv,
            drop_send,
            drop_recv,
        }
    }
}

#[derive(Resource, Clone)]
pub struct AssetServer {
    infos: Arc<asset_info::AssetInfos>,
    inner: Arc<AssetServerInner>,
}
impl Default for AssetServer {
    fn default() -> Self {
        Self {
            infos: Arc::new(asset_info::AssetInfos::default()),
            inner: Arc::default(),
        }
    }
}

impl AssetServer {
    /// Try to transition all asset state from [`asset::AssetState::*`] to [`asset::AssetState::Unloading`]
    /// from all [`asset::AssetIdUntyped`] submitted to the drop queue
    ///
    /// # Locking behavior
    /// Since all state is stored behind a RwLock shard, write will be attempted, but upon
    /// failure, will not be done and simply skipped.
    pub fn flush(&self) -> anyhow::Result<()> {
        while let Ok(drop_id) = self.inner.drop_recv.try_recv() {
            match self.infos.states.try_get_mut(&drop_id) {
                TryResult::Present(mut asset_info) => {
                    // order unloading to start
                    asset_info.asset_state = asset::AssetState::Unloading;
                }
                TryResult::Absent | TryResult::Locked => {
                    // ignore
                    continue;
                }
            }
        }
        Ok(())
    }

    pub fn get_deltas(&self) -> Vec<AssetServerDelta> {
        let mut deltas: Vec<AssetServerDelta> = Vec::new();
        while let Ok(delta) = self.inner.delta_recv.try_recv() {
            deltas.push(delta);
        }
        deltas
    }

    pub fn insert_resource<T: asset::Asset>(
        &self,
        metadata: T::Metadata,
    ) -> Option<asset::AssetHandle<T>> {
        let id_untyped: asset::AssetIdUntyped = {
            let handle = self.infos.handle_allocator.get_next_handle();
            asset::AssetIdUntyped::Generation {
                id: handle.index,
                generation: handle.generation,
                type_id: TypeId::of::<T>(),
            }
        };
        let arc = Arc::new(asset::StrongAssetHandleUntyped {
            id: id_untyped,
            drop_send: self.inner.drop_send.clone(),
        });
        if self.infos.states.get(&id_untyped).is_none() {
            // new handle made and subsequently loaded back
            self.infos
                .states
                .insert(id_untyped, asset_info::AssetInfo::new::<T>(&arc, metadata));
            self.inner
                .delta_send
                .send(AssetServerDelta::HandleLoaded(
                    asset::AssetHandleUntyped::Strong(arc.clone()),
                ))
                .unwrap();
            println!("In queue");
            Some(asset::AssetHandle::<T>::Strong(arc))
        } else {
            self.infos
                .handle_allocator
                .recycle(asset::InternalHandle::from(arc.id));
            None
        }
    }

    pub fn entry<T: asset::Asset>(&self, metadata: T::Metadata) -> asset::AssetHandle<T> {
        let id_untyped: asset::AssetIdUntyped = {
            let handle = self.infos.handle_allocator.get_next_handle();
            asset::AssetIdUntyped::Generation {
                id: handle.index,
                generation: handle.generation,
                type_id: TypeId::of::<T>(),
            }
        };
        self.infos
            .states
            .get_mut(&id_untyped)
            .map(|mut info| {
                info.handle
                    .upgrade()
                    .map(|arc| asset::AssetHandle::<T>::Strong(arc))
                    .unwrap_or({
                        // make a new handle, old one was dropped
                        let internal_handle = self.infos.handle_allocator.get_next_handle();
                        let untyped_handle = asset::AssetIdUntyped::Generation {
                            id: internal_handle.index,
                            generation: internal_handle.generation,
                            type_id: TypeId::of::<T>(),
                        };
                        let arc = Arc::new(asset::StrongAssetHandleUntyped {
                            id: untyped_handle.clone(),
                            drop_send: self.inner.drop_send.clone(),
                        });
                        info.handle = Arc::downgrade(&arc);
                        // new handle loaded, send it
                        self.inner
                            .delta_send
                            .send(AssetServerDelta::HandleLoaded(
                                asset::AssetHandleUntyped::Strong(arc.clone()),
                            ))
                            .unwrap();
                        asset::AssetHandle::<T>::Strong(arc)
                    })
            })
            .unwrap_or(self.insert_resource(metadata).unwrap())
            .clone()
    }

    /// Get metadata
    pub fn get_metadata<T: asset::Asset>(
        &self,
        handle: &asset::AssetHandleUntyped,
    ) -> Option<T::Metadata> {
        self.infos
            .states
            .get(&**handle)
            .map(|info| {
                info.metadata
                    .downcast_ref::<T::Metadata>()
                    .map(|d| d.clone())
            })
            .flatten()
    }
}
