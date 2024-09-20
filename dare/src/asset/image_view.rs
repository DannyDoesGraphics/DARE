use crate::asset::prelude as asset;
use async_stream::stream;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::resource;
use dagal::resource::traits::Resource;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;
use tokio::sync::watch::Sender;

#[derive(Debug, Clone)]
pub struct ImageView<A: Allocator + 'static> {
    _phantom: PhantomData<A>,
}

impl<A: Allocator + 'static> asset::AssetDescriptor for ImageView<A> {
    type Loaded = ImageViewLoaded;
    type Metadata = ImageViewMetadata<A>;
}

#[derive(Debug, PartialEq)]
pub struct ImageViewLoaded {
    pub handle: resource::ImageView,
}
impl Eq for ImageViewLoaded {}

#[derive(Debug, Clone)]
pub struct ImageViewMetadata<A: Allocator + 'static> {
    pub flags: vk::ImageViewCreateFlags,
    pub image: crate::asset::image::ImageMetaData<A>,
    pub view_type: vk::ImageViewType,
    pub format: vk::Format,
    pub components: vk::ComponentMapping,
    pub subresource_range: vk::ImageSubresourceRange,
}
impl<A: Allocator + 'static> Hash for ImageViewMetadata<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.flags.hash(state);
        self.image.hash(state);
        self.view_type.hash(state);
        self.format.hash(state);
    }
}
impl<A: Allocator + 'static> PartialEq for ImageViewMetadata<A> {
    fn eq(&self, other: &Self) -> bool {
        self.flags == other.flags
            && self.image == other.image
            && self.view_type == other.view_type
            && self.format == other.format
    }
}
impl<A: Allocator + 'static> Eq for ImageViewMetadata<A> {}

#[derive(Debug)]
pub struct ImageViewLoadInfo {
    pub device: dagal::device::LogicalDevice,
    pub vk_image: vk::Image,

    pub flags: vk::ImageViewCreateFlags,
    pub view_type: vk::ImageViewType,
    pub format: vk::Format,
    pub components: vk::ComponentMapping,
    pub subresource_range: vk::ImageSubresourceRange,
}

impl<A: Allocator + 'static> asset::AssetUnloaded for ImageViewMetadata<A> {
    type AssetLoaded = ImageViewLoaded;
    type Chunk = resource::ImageView;
    type StreamInfo = ImageViewLoadInfo;
    type LoadInfo = ImageViewLoadInfo;

    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<Self::Chunk>>> {
        let image_view = resource::ImageView::new(resource::ImageViewCreateInfo::FromCreateInfo {
            device: stream_info.device.clone(),
            create_info: vk::ImageViewCreateInfo {
                s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                p_next: ptr::null(),
                flags: stream_info.flags,
                image: stream_info.vk_image,
                view_type: stream_info.view_type,
                format: stream_info.format,
                components: stream_info.components,
                subresource_range: stream_info.subresource_range,
                _marker: Default::default(),
            },
        })?;
        Ok(Box::pin(stream! {
            yield Ok(image_view)
        }))
    }

    async fn load(
        &self,
        load_info: Self::LoadInfo,
        sender: Sender<Option<Arc<Self::AssetLoaded>>>,
    ) -> anyhow::Result<Arc<Self::AssetLoaded>> {
        let image_view = self
            .clone()
            .stream(load_info)
            .await?
            .next()
            .await
            .unwrap()?;
        let loaded = Arc::new(ImageViewLoaded { handle: image_view });
        #[cfg(feature = "tokio")]
        sender.send(Some(loaded.clone())).await?;
        #[cfg(not(feature = "tokio"))]
        sender.send(Some(loaded.clone()))?;
        Ok(loaded)
    }
}
