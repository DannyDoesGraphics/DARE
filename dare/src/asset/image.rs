use crate::asset::asset::AssetUnloaded;
use crate::asset::buffer::{BufferMetaData, BufferStreamInfo};
use crate::asset::format::Format;
use crate::render;
use anyhow::Result;
use async_stream::stream;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use futures::stream::BoxStream;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::watch::Sender;

pub struct Image<A: Allocator> {
    _maker: PhantomData<A>
}

impl<A: Allocator + 'static> super::asset::AssetDescriptor for Image<A> {
    type Loaded = ImageLoaded;
    type Metadata = ImageMetaData<A>;
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ImageLoaded {
    pub handle: vk::Image,
    pub format: image::ImageFormat,
}

#[derive(Debug, Clone)]
pub struct ImageMetaData<A: Allocator + 'static> {
    pub buffer: BufferMetaData<A>,
    pub image_format: image::ImageFormat,
}

impl<A: Allocator + 'static> PartialEq for ImageMetaData<A> {
    fn eq(&self, other: &Self) -> bool {
        self.buffer == other.buffer && self.image_format == other.image_format
    }
}

impl<A: Allocator + 'static> Hash for ImageMetaData<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.buffer.hash(state);
        self.image_format.hash(state);
    }
}

impl<A: Allocator + 'static> Eq for ImageMetaData<A> {}

#[derive(Debug, Clone)]
pub struct ImageStreamInfo {
    format: image::ImageFormat,
    chunk_size: usize,
}

pub struct ImageLoadInfo<A: Allocator + 'static> {
    allocator: ArcAllocator<A>,
    stream_info: BufferStreamInfo,
    transfer: render::transfer::TransferPool,
    target_format: Option<Format>,
    buffer_location: MemoryLocation,
}

impl<A: Allocator + 'static> AssetUnloaded for ImageMetaData<A> {
    type AssetLoaded = ImageLoaded;
    type Chunk = Pin<Vec<u8>>;
    type StreamInfo = ImageStreamInfo;
    type LoadInfo = ImageLoadInfo<A>;

    /// Image streaming **will load the entire image into memory**, but provide a stream of the pixels
    /// in bytes
    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> Result<BoxStream<'static, Result<Self::Chunk>>> {
        // Really not ideal, but we have to load the entire image
        let image = image::load_from_memory(&self.buffer.load_data().await?)?;
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

    async fn load(&self, load_info: Self::LoadInfo, sender: Sender<Option<Arc<Self::AssetLoaded>>>) -> Result<Arc<Self::AssetLoaded>> {
        let image = image::load_from_memory(&self.buffer.load_data().await?)?;
        let image = image.to_rgba8();
        let pixels = image
            .pixels()
            .flat_map(|pixel| pixel.0)
            .collect::<Vec<u8>>();
        

        todo!()
    }
}
