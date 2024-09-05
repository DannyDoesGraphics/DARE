use crate::asset::prelude as asset;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::resource;
use futures::stream::BoxStream;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

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
        self.flags == other.flags && self.image == other.image && self.view_type == other.view_type
            && self.format == other.format
    }
}
impl<A: Allocator + 'static> Eq for ImageViewMetadata<A> {}

impl<A: Allocator + 'static> asset::AssetUnloaded for ImageViewMetadata<A> {
    type AssetLoaded = ImageViewLoaded;
    type Chunk = vk::ImageView;
    type StreamInfo = asset::AssetManager<A>;

    async fn stream(self, stream_info: Self::StreamInfo) -> anyhow::Result<BoxStream<'static, anyhow::Result<Self::Chunk>>> {
        let image = stream_info.get_slot_loaded::<asset::Image<A>>(&self.image, Some()).await?;

        todo!()
    }
}