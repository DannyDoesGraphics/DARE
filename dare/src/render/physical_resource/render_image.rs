use crate::asset::loaders::MetaDataLoad;
use crate::asset::prelude::Asset;
use crate::prelude as dare;
use crate::render::physical_resource::traits::MetaDataRenderAsset;
use bevy_ecs::prelude::*;
use dagal::allocators::{Allocator, MemoryLocation};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use std::io::Read;
use std::ptr;

#[derive(Debug, Component)]
pub struct RenderImage<A: Allocator + 'static> {
    pub image: dagal::resource::Image<A>,
    pub full_view: dagal::resource::ImageView,
    pub handle: dare::asset::AssetHandle<dare::asset::assets::Image>,
}

impl<A: Allocator + 'static> MetaDataRenderAsset for RenderImage<A> {
    type Loaded = RenderImage<A>;
    type Asset = dare::asset::assets::Image;
    type PrepareInfo = (
        A,
        dare::asset::AssetHandle<dare::asset::assets::Image>,
        dare::render::util::TransferPool<A>,
        Option<String>,
    );

    fn prepare_asset(
        metadata: <Self::Asset as Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
    ) -> anyhow::Result<Self::Loaded> {
        todo!()
    }

    fn load_asset<'a>(
        metadata: <Self::Asset as Asset>::Metadata,
        prepare_info: Self::PrepareInfo,
        _: <<Self::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'_>,
    ) -> BoxFuture<'a, anyhow::Result<Self::Loaded>> {
        Box::pin(async move {
            let (mut allocator, handle, transfer_pool, name) = prepare_info;
            let image_loaded = metadata.load(()).await?.image.to_rgba8();
            let bytes: Vec<u8> = image_loaded
                .bytes()
                .map(|b| b.unwrap())
                .collect::<Vec<u8>>();
            let device = allocator.device();
            let mut staging_buffer =
                dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device: device.clone(),
                    name: name.clone(),
                    allocator: &mut allocator,
                    size: bytes.len() as vk::DeviceSize,
                    memory_type: MemoryLocation::CpuToGpu,
                    usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                        | vk::BufferUsageFlags::TRANSFER_DST,
                })?;
            staging_buffer.write(0, &bytes)?;

            let extent = vk::Extent3D {
                width: image_loaded.width(),
                height: image_loaded.height(),
                depth: 1,
            };
            let image =
                dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
                    device: allocator.device(),
                    allocator: &mut allocator,
                    location: MemoryLocation::GpuOnly,
                    image_ci: vk::ImageCreateInfo {
                        s_type: vk::StructureType::IMAGE_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::ImageCreateFlags::empty(),
                        image_type: vk::ImageType::TYPE_2D,
                        format: vk::Format::R8G8B8A8_SRGB,
                        extent,
                        mip_levels: 1,
                        array_layers: 1,
                        samples: vk::SampleCountFlags::TYPE_1,
                        tiling: vk::ImageTiling::OPTIMAL,
                        usage: vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                        sharing_mode: vk::SharingMode::CONCURRENT,
                        queue_family_index_count: device.get_used_queue_families().len() as u32,
                        p_queue_family_indices: device.get_used_queue_families().as_ptr(),
                        initial_layout: vk::ImageLayout::UNDEFINED,
                        _marker: Default::default(),
                    },
                    name: name.as_deref(),
                })?;
            // start transfer
            let mut stream = dare::render::physical_resource::gpu_texture_stream(
                staging_buffer,
                image,
                vk::ImageLayout::UNDEFINED,
                Some(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                transfer_pool,
                futures::stream::once(async move { Ok(bytes) }),
            )
            .boxed();
            while let Some(res) = stream.next().await {
                match res {
                    Some((staging, image)) => {
                        drop(staging);
                        let full_view = image.acquire_full_image_view()?;
                        tracing::trace!("Created image");
                        return Ok(Self {
                            image,
                            full_view,
                            handle,
                        });
                    }
                    None => {
                        // still processing
                    }
                }
            }
            unreachable!()
        })
    }
}

impl<A: Allocator> Drop for RenderImage<A> {
    fn drop(&mut self) {
        tracing::trace!("Dropping image");
    }
}
