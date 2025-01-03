use super::super::prelude as asset;

/// Deltas used to indicate changes in the asset manager
pub enum AssetServerDelta {
    HandleLoaded(asset::AssetHandleUntyped),
    HandleUnloaded(asset::AssetHandleUntyped),
}
unsafe impl Send for AssetServerDelta {}
