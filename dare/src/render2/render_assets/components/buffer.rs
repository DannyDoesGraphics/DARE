use crate::asset2::loaders::MetaDataStreamable;
use crate::prelude as dare;
use crate::render2::prelude::util::TransferPool;
use crate::render2::render_assets::gpu_stream::gpu_buffer_stream;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use bevy_ecs::prelude::Component;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dare::asset2 as asset;
use futures::StreamExt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// Describes a buffer used for rendering
#[derive(Component)]
pub struct RenderBuffer<A: Allocator + 'static> {
    pub buffer: dagal::resource::Buffer<A>,
    pub handle: asset::AssetHandle<asset::assets::Buffer>,
}
impl<A: Allocator + 'static> Deref for RenderBuffer<A> {
    type Target = dagal::resource::Buffer<A>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl<A: Allocator + 'static> DerefMut for RenderBuffer<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

pub struct BufferPrepareInfo<A: Allocator + 'static> {
    pub allocator: ArcAllocator<A>,
    pub handle: asset::AssetHandle<asset::assets::Buffer>,
    pub transfer_pool: TransferPool<A>,
    pub usage_flags: vk::BufferUsageFlags,
    pub location: MemoryLocation,
}

impl<A: Allocator + 'static> MetaDataRenderAsset for RenderBuffer<A> {
    type Loaded = RenderBuffer<A>;
    type Asset = asset::assets::Buffer;
    type PrepareInfo = BufferPrepareInfo<A>;

    fn prepare_asset(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        mut prepare_info: Self::PrepareInfo,
    ) -> anyhow::Result<Self::Loaded> {
        todo!()
    }

    async fn load_asset<'a>(
        metadata: <Self::Asset as asset::Asset>::Metadata,
        mut prepare_info: Self::PrepareInfo,
        load_info: <<Self::Asset as asset::Asset>::Metadata as asset::loaders::MetaDataLoad>::LoadInfo<'a>,
    ) -> anyhow::Result<Self::Loaded>
    where
        A: 'a,
    {
        let destination =
            dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                name: Some(String::from("BufferAsset")),
                device: prepare_info.allocator.device().clone(),
                allocator: &mut prepare_info.allocator,
                size: (metadata.element_count * metadata.format.size()) as vk::DeviceSize,
                memory_type: prepare_info.location,
                usage_flags: vk::BufferUsageFlags::TRANSFER_DST | prepare_info.usage_flags,
            })?;
        let stream = metadata
            .stream(asset::assets::BufferStreamInfo {
                chunk_size: load_info.chunk_size,
            })
            .await?;
        let transfer_pool = prepare_info.transfer_pool.clone();
        let staging_buffer =
            dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: prepare_info.allocator.get_device().clone(),
                name: Some(String::from("Staging Buffer")),
                allocator: &mut prepare_info.allocator,
                size: load_info.chunk_size as vk::DeviceSize,
                memory_type: MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST,
            })?;
        let mut stream =
            gpu_buffer_stream(staging_buffer, destination, transfer_pool, stream).boxed();

        while let Some(res) = stream.next().await {
            match res {
                Ok(Some((staging, dest))) => {
                    drop(staging);
                    return Ok(Self {
                        buffer: dest,
                        handle: prepare_info.handle,
                    });
                }
                Ok(None) => {
                    // still processing
                }
                Err(e) => {
                    // destroy buffer
                    //drop(destination);
                    return Err(e);
                }
            }
        }
        unreachable!();
    }
}
