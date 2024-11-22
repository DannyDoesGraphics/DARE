pub mod asset_info;
pub mod deltas;

use super::prelude as asset;
use bevy_ecs::prelude::*;
use dare_containers::dashmap::try_result::TryResult;
pub use deltas::AssetServerDelta;
use std::any::TypeId;
use std::hash::{Hash, Hasher};
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
            asset::AssetIdUntyped::MetadataHash {
                id: {
                    let mut hasher = std::hash::DefaultHasher::default();
                    metadata.hash(&mut hasher);
                    hasher.finish()
                },
                type_id: TypeId::of::<T>(),
            }
        };

        if self.infos.states.get(&id_untyped).is_none() {
            // new handle made and subsequently loaded back
            let arc = Arc::new(asset::StrongAssetHandleUntyped {
                id: id_untyped,
                drop_send: self.inner.drop_send.clone(),
            });
            println!("Was forced to make: {:?} = {}", metadata, {
                let mut hasher = std::hash::DefaultHasher::default();
                metadata.hash(&mut hasher);
                hasher.finish()
            });
            self.infos
                .states
                .insert(id_untyped, asset_info::AssetInfo::new::<T>(&arc, metadata));
            self.inner
                .delta_send
                .send(AssetServerDelta::HandleLoaded(
                    asset::AssetHandleUntyped::Strong(arc.clone()),
                ))
                .unwrap();
            Some(asset::AssetHandle::<T>::Strong(arc))
        } else {
            if matches!(id_untyped, asset::AssetIdUntyped::Generation { .. }) {
                self.infos
                    .handle_allocator
                    .recycle(asset::InternalHandle::from(id_untyped));
            }
            None
        }
    }

    pub fn entry<T: asset::Asset>(&self, metadata: T::Metadata) -> asset::AssetHandle<T> {
        let id_untyped: asset::AssetIdUntyped = {
            asset::AssetIdUntyped::MetadataHash {
                id: {
                    let mut hasher = std::hash::DefaultHasher::default();
                    metadata.hash(&mut hasher);
                    hasher.finish()
                },
                type_id: TypeId::of::<T>(),
            }
        };
        if self.infos.states.get(&id_untyped).is_none() {
            self.insert_resource(metadata).unwrap()
        } else if let Some(handle) = {
            match self.infos.states.get(&id_untyped) {
                None => None,
                Some(info) => info.handle.upgrade(),
            }
        } {
            asset::AssetHandle::<T>::Strong(handle)
        } else if {
            let info = self.infos.states.get(&id_untyped).unwrap();
            info.handle.upgrade().is_none()
        } {
            let mut info = self.infos.states.get_mut(&id_untyped).unwrap();
            // make a new handle, old one was dropped
            let arc = Arc::new(asset::StrongAssetHandleUntyped {
                id: id_untyped,
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
            println!("bro {:?}", metadata);
            asset::AssetHandle::<T>::Strong(arc)
        } else {
            panic!()
        }
        /*
        self.infos
            .states
            .get_mut(&id_untyped)
            .map(|mut info| {
                info.handle
                    .upgrade()
                    .map(|arc| asset::AssetHandle::<T>::Strong(arc))
                    .unwrap_or({
                        // make a new handle, old one was dropped
                        let arc = Arc::new(asset::StrongAssetHandleUntyped {
                            id: id_untyped,
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
                        println!("bro {:?}", metadata);
                        asset::AssetHandle::<T>::Strong(arc)
                    })
            })
            .unwrap_or(self.insert_resource(metadata).unwrap())
            .clone()*/
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

    /// Update state forcefully
    pub unsafe fn update_state(
        &self,
        handle: &asset::AssetIdUntyped,
        state: asset::AssetState,
    ) -> Option<()> {
        self.infos.states.get_mut(&handle).map(|mut info| {
            info.asset_state = state;
        })?;
        Some(())
    }

    pub fn get_state(&self, handle: &asset::AssetIdUntyped) -> Option<asset::AssetState> {
        self.infos.states.get(&handle).map(|info| info.asset_state)
    }
}
