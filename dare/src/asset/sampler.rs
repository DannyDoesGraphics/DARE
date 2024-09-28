use crate::asset::prelude::AssetDescriptor;
use crate::prelude as dare;
use anyhow::Error;
use async_stream::stream;
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use futures::stream::BoxStream;
use std::hash::Hasher;
use std::ptr;
use std::sync::Arc;
use tokio::sync::watch::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampler {}

impl AssetDescriptor for Sampler {
    type Loaded = dagal::resource::Sampler;
    type Metadata = SamplerMetaData;
}

#[derive(Debug, PartialEq, Clone)]
pub struct SamplerLoadInfo {
    pub device: dagal::device::LogicalDevice,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerMetaData {
    pub flags: vk::SamplerCreateFlags,
    pub mag_filter: vk::Filter,
    pub min_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_mode_u: vk::SamplerAddressMode,
    pub address_mode_v: vk::SamplerAddressMode,
    pub address_mode_w: vk::SamplerAddressMode,
    pub mip_lod_bias: f32,
    pub anisotropy_enable: bool,
    pub max_anisotropy: f32,
    pub compare_enable: bool,
    pub compare_op: vk::CompareOp,
    pub min_lod: f32,
    pub max_lod: f32,
    pub border_color: vk::BorderColor,
    pub unnormalized_coordinates: bool,
}
impl Eq for SamplerMetaData {}
impl std::hash::Hash for SamplerMetaData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.flags.hash(state);
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_mode.hash(state);
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.anisotropy_enable.hash(state);
        self.compare_enable.hash(state);
        self.compare_op.hash(state);
        self.border_color.hash(state);
        self.unnormalized_coordinates.hash(state);
    }
}

impl dare::asset::AssetUnloaded for SamplerMetaData {
    type AssetLoaded = dagal::resource::Sampler;
    type Chunk = Arc<dagal::resource::Sampler>;
    type StreamInfo = SamplerLoadInfo;
    type LoadInfo = SamplerLoadInfo;

    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<Self::Chunk>>> {
        Ok(Box::pin(stream! {
                let sampler_ci = vk::SamplerCreateInfo {
                s_type: vk::StructureType::SAMPLER_CREATE_INFO,
                p_next: ptr::null(),
                flags: self.flags,
                mag_filter: self.mag_filter,
                min_filter: self.min_filter,
                mipmap_mode: self.mipmap_mode,
                address_mode_u: self.address_mode_u,
                address_mode_v: self.address_mode_v,
                address_mode_w: self.address_mode_w,
                mip_lod_bias: self.mip_lod_bias,
                anisotropy_enable: vk::Bool32::from(self.anisotropy_enable),
                max_anisotropy: self.max_anisotropy,
                compare_enable: vk::Bool32::from(self.compare_enable),
                compare_op: self.compare_op,
                min_lod: self.min_lod,
                max_lod: self.max_lod,
                border_color: self.border_color,
                unnormalized_coordinates: vk::Bool32::from(self.unnormalized_coordinates),
                _marker: Default::default(),
            };
            let sampler = dagal::resource::Sampler::new(
                dagal::resource::SamplerCreateInfo::FromCreateInfo {
                    device: stream_info.device,
                    create_info: sampler_ci,
                    name: None,
                }
            ).and_then(|sampler| Ok(Arc::new(sampler)))?;
            yield Ok(sampler)
        }))
    }

    async fn load(
        &self,
        load_info: Self::LoadInfo,
        sender: Sender<Option<Arc<Self::AssetLoaded>>>,
    ) -> anyhow::Result<Arc<Self::AssetLoaded>> {
        let sampler_ci = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SAMPLER_CREATE_INFO,
            p_next: ptr::null(),
            flags: self.flags,
            mag_filter: self.mag_filter,
            min_filter: self.min_filter,
            mipmap_mode: self.mipmap_mode,
            address_mode_u: self.address_mode_u,
            address_mode_v: self.address_mode_v,
            address_mode_w: self.address_mode_w,
            mip_lod_bias: self.mip_lod_bias,
            anisotropy_enable: vk::Bool32::from(self.anisotropy_enable),
            max_anisotropy: self.max_anisotropy,
            compare_enable: vk::Bool32::from(self.compare_enable),
            compare_op: self.compare_op,
            min_lod: self.min_lod,
            max_lod: self.max_lod,
            border_color: self.border_color,
            unnormalized_coordinates: vk::Bool32::from(self.unnormalized_coordinates),
            _marker: Default::default(),
        };
        let sampler =
            dagal::resource::Sampler::new(dagal::resource::SamplerCreateInfo::FromCreateInfo {
                device: load_info.device,
                create_info: sampler_ci,
                name: None,
            })
            .and_then(|sampler| Ok(Arc::new(sampler)));
        match sampler {
            Ok(sampler) => Ok(sampler),
            Err(e) => {
                sender.send(None)?;
                Err(e)
            }
        }
    }
}
