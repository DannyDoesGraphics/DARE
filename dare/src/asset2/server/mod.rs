pub mod asset_info;
pub mod deltas;
pub mod render_asset_state;

use super::prelude as asset;
use bevy_ecs::prelude::*;
use dare_containers::dashmap::try_result::TryResult;
pub use deltas::AssetServerDelta;
use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(thiserror::Error, Debug, Copy, Clone)]
pub enum AssetServerErrors {
    #[error("Expected state unloaded, got {0:?}. Expected {1:?}")]
    UnexpectedAssetState(asset::AssetState, asset::AssetState),
    #[error("Asset handle {0:?} does not exist.")]
    NullHandle(asset::AssetIdUntyped),
}

/// Batched state change for efficient processing
#[derive(Debug, Clone)]
pub struct StateChange {
    pub id: asset::AssetIdUntyped,
    pub new_state: asset::AssetState,
}

/// Asset manager (engine side) - optimized for high performance
#[derive(Debug)]
pub struct AssetServerInner {
    delta_send: crossbeam_channel::Sender<AssetServerDelta>,
    delta_recv: crossbeam_channel::Receiver<AssetServerDelta>,
    /// Cloned and handed out to [`asset::StrongAssetHandleUntyped`] to be sent upon struct being
    /// dropped and adds it to the queue to set the asset state to be unloaded.
    drop_send: crossbeam_channel::Sender<asset::AssetIdUntyped>,
    /// Receives all drop requests
    drop_recv: crossbeam_channel::Receiver<asset::AssetIdUntyped>,
    /// Channel for batching state changes
    state_changes_send: crossbeam_channel::Sender<StateChange>,
    state_changes_recv: crossbeam_channel::Receiver<StateChange>,
    /// Generation counter for cache invalidation
    generation: AtomicU64,
}

impl Default for AssetServerInner {
    fn default() -> Self {
        let (delta_send, delta_recv) = crossbeam_channel::unbounded();
        let (drop_send, drop_recv) = crossbeam_channel::unbounded();
        let (state_changes_send, state_changes_recv) = crossbeam_channel::unbounded();
        Self {
            delta_send,
            delta_recv,
            drop_send,
            drop_recv,
            state_changes_send,
            state_changes_recv,
            generation: AtomicU64::new(0),
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
    pub fn try_flush(&self) -> anyhow::Result<()> {
        // Collect all drop requests first
        let mut drop_requests = Vec::new();
        while let Ok(drop_id) = self.inner.drop_recv.try_recv() {
            drop_requests.push(drop_id);
        }

        // Process drops in batch - convert to state changes
        for drop_id in drop_requests {
            if let Err(_) = self.inner.state_changes_send.try_send(StateChange {
                id: drop_id,
                new_state: asset::AssetState::Unloading,
            }) {
                // Channel full, which is unusual but non-critical
                tracing::warn!("State changes channel is full, skipping drop request");
            }
        }

        // Process all queued state changes
        let mut changes_applied = 0;
        let max_batch_size = 256; // Prevent excessive processing in one frame

        while changes_applied < max_batch_size {
            match self.inner.state_changes_recv.try_recv() {
                Ok(change) => {
                    // Try to apply the state change - if locked, queue it back
                    match self.infos.states.try_get_mut(&change.id) {
                        TryResult::Present(mut asset_info) => {
                            asset_info.asset_state = change.new_state;
                            changes_applied += 1;
                        }
                        TryResult::Absent => {
                            // Asset was removed, ignore
                        }
                        TryResult::Locked => {
                            // Re-queue for next frame instead of silently dropping
                            if let Err(_) = self.inner.state_changes_send.try_send(change) {
                                // If we can't re-queue, we'll have to drop it this time
                                tracing::warn!("Unable to re-queue locked state change");
                            }
                            break; // Exit to prevent infinite loops
                        }
                    }
                }
                Err(_) => break, // No more changes to process
            }
        }

        // Increment generation for cache invalidation
        if changes_applied > 0 {
            self.inner.generation.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Get current generation for cache invalidation
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Relaxed)
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
            self.infos
                .states
                .insert(id_untyped, asset_info::AssetInfo::new::<T>(&arc, metadata));
            let handle = asset::AssetHandle::<T>::Strong(arc);
            self.inner
                .delta_send
                .send(AssetServerDelta::HandleCreated(
                    handle.clone().downgrade().into_untyped_handle(),
                ))
                .unwrap();
            Some(handle)
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
                .send(AssetServerDelta::HandleCreated(
                    asset::AssetHandleUntyped::Weak {
                        id: id_untyped,
                        weak_ref: Arc::downgrade(&arc),
                    },
                ))
                .unwrap();
            asset::AssetHandle::<T>::Strong(arc)
        } else {
            panic!()
        }
    }

    pub fn get_metadata<T: asset::Asset>(
        &self,
        handle: &asset::AssetHandle<T>,
    ) -> Option<T::Metadata> {
        self.infos
            .states
            .get(&handle.clone().into_untyped_handle())
            .map(|info| {
                info.metadata
                    .downcast_ref::<T::Metadata>()
                    .map(|d| d.clone())
            })
            .flatten()
    }

    /// Get metadata
    pub fn get_metadata_untyped<T: asset::Asset>(
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
    ///
    /// Returns `None` if asset handle does not exist.
    pub unsafe fn update_state(
        &self,
        handle: &asset::AssetIdUntyped,
        state: asset::AssetState,
    ) -> Option<()> {
        // Check if the asset exists first
        if !self.infos.states.contains_key(handle) {
            return None;
        }

        // Queue the state change for batched processing
        if let Err(_) = self.inner.state_changes_send.try_send(StateChange {
            id: handle.clone(),
            new_state: state,
        }) {
            // Channel full - fall back to immediate processing
            tracing::warn!("State changes channel full, applying immediately");
            match self.infos.states.try_get_mut(handle) {
                TryResult::Present(mut asset_info) => {
                    asset_info.asset_state = state;
                }
                _ => return None,
            }
        }

        // Handle delta notifications immediately for critical state changes
        if let Some(info) = self.infos.states.get(handle) {
            if let Some(handle_arc) = info.handle.upgrade() {
                match &state {
                    asset::AssetState::Loading => {
                        if let Err(e) = self.inner.delta_send.send(AssetServerDelta::HandleLoading(
                            asset::AssetHandleUntyped::Weak {
                                id: handle_arc.id,
                                weak_ref: Arc::downgrade(&handle_arc),
                            },
                        )) {
                            tracing::error!("Failed to send delta: {:?}", e);
                        }
                    }
                    asset::AssetState::Unloading => {
                        if let Err(e) =
                            self.inner
                                .delta_send
                                .send(AssetServerDelta::HandleUnloading(
                                    asset::AssetHandleUntyped::Weak {
                                        id: handle_arc.id,
                                        weak_ref: Arc::downgrade(&handle_arc),
                                    },
                                ))
                        {
                            tracing::error!("Failed to send delta: {:?}", e);
                        }
                    }
                    _ => {} // Other states don't need immediate delta notifications
                }
            }
        }

        Some(())
    }

    /// Attempt to transition an asset from unloaded -> loading
    pub fn transition_loading(
        &self,
        handle: &asset::AssetIdUntyped,
    ) -> Result<(), AssetServerErrors> {
        match self.get_state(handle) {
            None => Err(AssetServerErrors::NullHandle(handle.clone())),
            Some(found_state) => {
                if matches!(found_state, asset::AssetState::Unloaded) {
                    unsafe {
                        self.update_state(handle, asset::AssetState::Loading);
                        Ok(())
                    }
                } else {
                    Err(AssetServerErrors::UnexpectedAssetState(
                        found_state,
                        asset::AssetState::Unloaded,
                    ))
                }
            }
        }
    }

    pub fn get_state(&self, handle: &asset::AssetIdUntyped) -> Option<asset::AssetState> {
        self.infos.states.get(&handle).map(|info| info.asset_state)
    }
}
