use super::super::prelude as asset;
use dare_containers::dashmap::DashMap;
use std::any::Any;
use std::sync::{Arc, Weak};

/// Responsible for an individual asset's state
pub struct AssetInfo {
    pub(super) asset_state: asset::AssetState,
    pub(super) handle: Weak<asset::StrongAssetHandleUntyped>,
    pub(super) metadata: Arc<Box<dyn Any + 'static + Send + Sync>>,
}

impl AssetInfo {
    /// Make a new asset info struct
    pub fn new<T: asset::Asset>(
        handle: &Arc<asset::StrongAssetHandleUntyped>,
        metadata: T::Metadata,
    ) -> Self {
        Self {
            asset_state: asset::AssetState::Unloaded,
            handle: Arc::downgrade(handle),
            metadata: Arc::new(Box::new(metadata)),
        }
    }
}

pub struct AssetInfos {
    pub(super) states: DashMap<asset::AssetIdUntyped, AssetInfo>,
    pub(super) handle_allocator: super::super::handle_allocator::HandleAllocator,
}

impl Default for AssetInfos {
    fn default() -> Self {
        Self {
            states: DashMap::new(),
            handle_allocator: Default::default(),
        }
    }
}
