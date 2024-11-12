use crate::prelude as dare;
use dare::asset2 as asset;
use std::fmt::Debug;

pub trait MetaDataRenderAsset {
    type Loaded;
    type Asset: asset::Asset;
    type PrepareInfo: Send;

    /// Prepares the asset's contents to be loaded in
    fn prepare_asset(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
    ) -> anyhow::Result<Self::Loaded>;

    /// Given the readied asset, load into it
    async fn load_asset(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
        load_info: <<Self::Asset as asset::Asset>::Metadata as asset::loaders::MetaDataLoad>::LoadInfo<'_>,
    ) -> anyhow::Result<Self::Loaded>;
}
