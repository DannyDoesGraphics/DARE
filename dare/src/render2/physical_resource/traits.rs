use crate::prelude as dare;
use dare::asset2 as asset;
use futures_core::future::BoxFuture;

pub trait MetaDataRenderAsset: 'static {
    type Loaded: Send;
    type Asset: asset::Asset;
    type PrepareInfo: Send;

    /// Prepares the asset's contents to be loaded in
    fn prepare_asset(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
    ) -> anyhow::Result<Self::Loaded>;

    /// Given the readied asset, load into it
    fn load_asset<'a>(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
        load_info: <<Self::Asset as asset::Asset>::Metadata as asset::loaders::MetaDataLoad>::LoadInfo<'_>,
    ) -> BoxFuture<'a, anyhow::Result<Self::Loaded>>;
}
