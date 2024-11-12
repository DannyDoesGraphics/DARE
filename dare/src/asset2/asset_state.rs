use crate::asset2::prelude as asset;
use std::sync::Arc;

#[derive(Debug)]
pub enum AssetState {
    /// Asset is being unloaded
    Unloaded,
    /// Asset is loading currently
    Loading,
    /// Asset loaded
    Loaded,
    /// Unloading asset
    Unloading,
    /// Asset failed
    Failed,
}