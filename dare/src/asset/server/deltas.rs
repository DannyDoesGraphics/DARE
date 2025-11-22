use super::super::prelude as asset;

/// Deltas used to indicate changes in the asset manager
pub enum AssetServerDelta {
    HandleCreated(asset::AssetHandleUntyped),
    HandleLoading(asset::AssetHandleUntyped),
    HandleUnloading(asset::AssetHandleUntyped),
    HandleDestroyed(asset::AssetHandleUntyped),
}
unsafe impl Send for AssetServerDelta {}
