use std::ptr;
use dagal::allocators::{Allocator, MemoryLocation};
use bevy_ecs::prelude::*;
use futures_core::future::BoxFuture;
use tracing::instrument::WithSubscriber;
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use crate::asset2::loaders::MetaDataLoad;
use crate::asset2::prelude::Asset;
use crate::prelude as dare;
use crate::render2::render_assets::traits::MetaDataRenderAsset;

#[derive(Debug, Component)]
pub struct Image<A: Allocator + 'static> {
    pub image: dagal::resource::Image<A>,
    pub handle: dare::asset2::AssetHandle<
        dare::asset2::assets::Image
    >
}

impl<A: Allocator + 'static> MetaDataRenderAsset for Image<A> {
    type Loaded = Image<A>;
    type Asset = dare::asset2::assets::Image;
    type PrepareInfo = (dagal::device::LogicalDevice, dagal::allocators::ArcAllocator<A>, dare::render::util::TransferPool<A>, u32);

    fn prepare_asset(metadata: <Self::Asset as Asset>::Metadata, prepare_info: Self::PrepareInfo) -> anyhow::Result<Self::Loaded> {
        todo!()
    }

    fn load_asset<'a>(metadata: <Self::Asset as Asset>::Metadata, prepare_info: Self::PrepareInfo, load_info: <<Self::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'_>) -> BoxFuture<'a, anyhow::Result<Self::Loaded>> {
        Box::pin(async move {
            let (device, mut allocator, transfer_pool, queue_family) = prepare_info;
            let image_loaded = metadata.load(()).await?;
            let image = unsafe {
                dagal::resource::Image::new(
                    dagal::resource::ImageCreateInfo::NewAllocated {
                        device,
                        queue_family: Some(0),
                        allocator: &mut allocator,
                        location: MemoryLocation::GpuOnly,
                        image_ci: vk::ImageCreateInfo {
                            s_type: vk::StructureType::IMAGE_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::ImageCreateFlags::empty(),
                            image_type: vk::ImageType::TYPE_2D,
                            format: vk::Format::R8G8B8A8_SRGB,
                            extent: vk::Extent3D {
                                width: image_loaded.image.width(),
                                height: image_loaded.image.height(),
                                depth: 1,
                            },
                            mip_levels: 1,
                            array_layers: 1,
                            samples: vk::SampleCountFlags::TYPE_1,
                            tiling: vk::ImageTiling::OPTIMAL,
                            usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                            sharing_mode: vk::SharingMode::EXCLUSIVE,
                            queue_family_index_count: 1,
                            p_queue_family_indices: &queue_family,
                            initial_layout: vk::ImageLayout::UNDEFINED,
                            _marker: Default::default(),
                        },
                        name: None,
                    }
                )
            };
            todo!()
        })
    }
}