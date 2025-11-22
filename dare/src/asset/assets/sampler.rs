use super::super::prelude as asset;
use crate::asset::loaders::MetaDataLoad;
use crate::asset::prelude::Asset;
use crate::render::physical_resource::traits::MetaDataRenderAsset;
use bevy_tasks::futures_lite::FutureExt;
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use futures::future::BoxFuture;
use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

pub struct Sampler {}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SamplerAsset {
    pub wrapping_mode: (WrappingMode, WrappingMode),
    pub min_filter: MinFilter,
    pub mag_filter: MagFilter,
}
impl Hash for SamplerAsset {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use gltf::texture::WrappingMode;
        match self.wrapping_mode.0 {
            WrappingMode::ClampToEdge => 0.hash(state),
            WrappingMode::MirroredRepeat => 1.hash(state),
            WrappingMode::Repeat => 2.hash(state),
        }
        match self.wrapping_mode.1 {
            WrappingMode::ClampToEdge => 0.hash(state),
            WrappingMode::MirroredRepeat => 1.hash(state),
            WrappingMode::Repeat => 2.hash(state),
        };
        use gltf::texture::MinFilter;
        match self.min_filter {
            MinFilter::Nearest => 0.hash(state),
            MinFilter::Linear => 1.hash(state),
            MinFilter::NearestMipmapNearest => 2.hash(state),
            MinFilter::LinearMipmapNearest => 3.hash(state),
            MinFilter::NearestMipmapLinear => 4.hash(state),
            MinFilter::LinearMipmapLinear => 5.hash(state),
        };
        use gltf::texture::MagFilter;
        match self.mag_filter {
            MagFilter::Nearest => 0.hash(state),
            MagFilter::Linear => 1.hash(state),
        };
    }
}
impl asset::AssetMetadata for SamplerAsset {}
impl asset::AssetLoaded for SamplerAsset {}

impl MetaDataLoad for SamplerAsset {
    type Loaded = SamplerAsset;

    type LoadInfo<'a> = ();

    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded> {
        unimplemented!()
    }
}

impl Asset for Sampler {
    type Metadata = SamplerAsset;
    type Loaded = SamplerAsset;
}

impl MetaDataRenderAsset for SamplerAsset {
    type Loaded = dagal::resource::Sampler;
    type Asset = Sampler;
    type PrepareInfo = (dagal::device::LogicalDevice, Option<String>);

    fn prepare_asset(
        metadata: <Self::Asset as Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
    ) -> anyhow::Result<Self::Loaded> {
        unimplemented!()
    }

    fn load_asset<'a>(
        metadata: <Self::Asset as Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
        _load_info: <<Self::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'_>,
    ) -> BoxFuture<'a, anyhow::Result<Self::Loaded>> {
        async move {
            let (device, name) = prepare_info;
            dagal::resource::Sampler::new(dagal::resource::SamplerCreateInfo::FromCreateInfo {
                device,
                name: name.as_deref(),
                flags: vk::SamplerCreateFlags::empty(),
                mag_filter: match metadata.mag_filter {
                    MagFilter::Nearest => vk::Filter::NEAREST,
                    MagFilter::Linear => vk::Filter::LINEAR,
                },
                min_filter: match metadata.min_filter {
                    MinFilter::Nearest => vk::Filter::NEAREST,
                    MinFilter::Linear => vk::Filter::LINEAR,
                    MinFilter::NearestMipmapNearest => vk::Filter::NEAREST,
                    MinFilter::LinearMipmapNearest => vk::Filter::LINEAR,
                    MinFilter::NearestMipmapLinear => vk::Filter::NEAREST,
                    MinFilter::LinearMipmapLinear => vk::Filter::LINEAR,
                },
                mipmap_mode: match metadata.mag_filter {
                    MagFilter::Nearest => vk::SamplerMipmapMode::NEAREST,
                    MagFilter::Linear => vk::SamplerMipmapMode::LINEAR,
                },
                address_mode_u: match metadata.wrapping_mode.0 {
                    WrappingMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    WrappingMode::MirroredRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
                    WrappingMode::Repeat => vk::SamplerAddressMode::REPEAT,
                },
                address_mode_v: match metadata.wrapping_mode.1 {
                    WrappingMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
                    WrappingMode::MirroredRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
                    WrappingMode::Repeat => vk::SamplerAddressMode::REPEAT,
                },
                address_mode_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                mip_lod_bias: 0.0,
                anisotropy_enable: 0,
                max_anisotropy: 0.0,
                compare_enable: 0,
                compare_op: Default::default(),
                min_lod: 0.0,
                max_lod: 0.0,
                border_color: Default::default(),
                unnormalized_coordinates: 0,
            })
            .map_err(|e| e.into())
        }
        .boxed()
    }
}
