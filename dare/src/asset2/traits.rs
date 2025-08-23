use super::prelude as asset;
use std::fmt::Debug;
use std::hash::Hash;

/// Describes metadata about the asset
pub trait AssetMetadata: Hash + Sized + Clone + Send + Sync + 'static {}

/// Describes the loaded asset
pub trait AssetLoaded: Debug + PartialEq + Eq {}

pub trait Asset: 'static {
    /// Asset unloaded form
    type Metadata: AssetMetadata + asset::loaders::MetaDataLoad<Loaded = Self::Loaded>;
    /// Asset loaded form
    type Loaded: AssetLoaded;
}
