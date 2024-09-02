use crate::asset::asset::AssetUnloaded;
use crate::asset::buffer::BufferMetaData;
use async_stream::stream;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use futures::stream::BoxStream;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::pin::Pin;

pub struct Image<A: Allocator> {
    _maker: PhantomData<A>
}

impl<A: Allocator> super::asset::AssetDescriptor for Image<A> {
    type Loaded = ImageLoaded;
    type Metadata = ImageMetaData<A>;
}

#[derive(PartialEq)]
pub struct ImageLoaded {
    pub handle: vk::Image,
    pub format: image::ImageFormat,
}

#[derive(Debug, Clone)]
pub struct ImageMetaData<A: Allocator> {
    pub buffer: BufferMetaData<A>,
    pub image_format: image::ImageFormat,
}

impl<A: Allocator> PartialEq for ImageMetaData<A> {
    fn eq(&self, other: &Self) -> bool {
        self.buffer == other.buffer && self.image_format == other.image_format
    }
}

impl<A: Allocator> Hash for ImageMetaData<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.buffer.hash(state);
        self.image_format.hash(state);
    }
}

impl<A: Allocator> Eq for ImageMetaData<A> {}

pub struct ImageLoadInfo {
    format: image::ImageFormat,
    chunk_size: usize,
}

impl<A: Allocator + 'static> AssetUnloaded for ImageMetaData<A> {
    type AssetLoaded = ImageLoaded;
    type Chunk = Pin<Vec<u8>>;
    type StreamInfo = ImageLoadInfo;

    /// Image streaming **will load the entire image into memory**, but provide a stream of the pixels
    /// in bytes
    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<Self::Chunk>>> {
        // Really not ideal, but we have to load the entire image
        let image = image::load_from_memory(&self.buffer.load().await?)?;
        let image = image.to_rgba8();
        let pixels = image
            .pixels()
            .flat_map(|pixel| pixel.0)
            .collect::<Vec<u8>>();
        Ok(Box::pin(stream! {
            for chunk in pixels.chunks(stream_info.chunk_size) {
                yield Ok(Pin::new(chunk.to_vec()));
            }
        }))
    }
}
