use crate::asset::asset::AssetUnloaded;
use crate::asset::buffer::BufferMetaData;
use crate::asset::format::{ElementFormat, Format};
use crate::render;
use crate::render::transfer::{ImageTransferRequest, TransferRequest};
use anyhow::Result;
use async_stream::stream;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use futures::stream::BoxStream;
use image::GenericImageView;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::pin::Pin;
use std::ptr;
use std::sync::Arc;
use tokio::sync::watch::Sender;

pub struct Image<A: Allocator> {
    _marker: PhantomData<A>
}

impl<A: Allocator + 'static> super::asset::AssetDescriptor for Image<A> {
    type Loaded = ImageLoaded<A>;
    type Metadata = ImageMetaData<A>;
}

#[derive(Debug)]
pub struct ImageLoaded<A: Allocator> {
    pub handle: resource::Image<A>,
    pub format: Format,
}

impl<A: Allocator> PartialEq for ImageLoaded<A> {
    fn eq(&self, other: &Self) -> bool {
        let eq = unsafe { self.handle.as_raw() == other.handle.as_raw() };
        eq && self.format == other.format
    }
}

impl<A: Allocator> Eq for ImageLoaded<A> {}

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
    transfer: render::transfer::TransferPool,
    target_format: Option<Format>,
    buffer_location: MemoryLocation,
    flags: vk::ImageCreateFlags,
    mip_levels: u32,
    array_layers: u32,
    samples: vk::SampleCountFlags,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    sharing_mode: vk::SharingMode,
    queues: Vec<dagal::device::Queue>,
    initial_layout: vk::ImageLayout,
}

impl<A: Allocator + 'static> AssetUnloaded for ImageMetaData<A> {
    type AssetLoaded = ImageLoaded<A>;
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
        // TODO: it is in theory possible to stream the image from CPU memory -> CpuToGpu -> GpuOnly buffers
        // TODO: streaming can be done by chunking down
        let image = image::load_from_memory(&self.buffer.load_data().await?)?;
        let image = image.to_rgba8();
        let image_size = image.height() * image.width() * 4;
        let image_extent = vk::Extent3D {
            width: image.width(),
            height: image.height(),
            depth: 1,
        };
        let pixels = image
            .pixels()
            .flat_map(|pixel| pixel.0)
            .collect::<Vec<u8>>();
        let mut allocator = load_info.allocator.clone();
        let transfer_buffer = resource::Buffer::new(
            resource::BufferCreateInfo::NewEmptyBuffer {
                device: load_info.allocator.device(),
                allocator: &mut allocator,
                size: pixels.len() as vk::DeviceSize,
                memory_type: MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
            }
        )?;
        let family_indices = load_info.queues.iter().map(|queue| queue.get_family_index()).collect::<HashSet<u32>>()
                                      .into_iter().collect::<Vec<u32>>();
        let dst_image = resource::Image::new(
            resource::ImageCreateInfo::NewAllocated {
                device: allocator.device(),
                allocator: &mut allocator,
                location: load_info.buffer_location,
                image_ci: vk::ImageCreateInfo {
                    s_type: vk::StructureType::IMAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: load_info.flags,
                    image_type: vk::ImageType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    extent: image_extent,
                    mip_levels: load_info.mip_levels,
                    array_layers: load_info.array_layers,
                    samples: load_info.samples,
                    tiling: load_info.tiling,
                    usage: load_info.usage | vk::ImageUsageFlags::TRANSFER_DST,
                    sharing_mode: load_info.sharing_mode,
                    queue_family_index_count: family_indices.len() as u32,
                    p_queue_family_indices: family_indices.as_ptr(),
                    initial_layout: load_info.initial_layout,
                    _marker: Default::default(),
                },
                name: None,
            }
        )?;
        unsafe {
            load_info.transfer.transfer_gpu(TransferRequest::Image(ImageTransferRequest {
                src_buffer: *transfer_buffer.as_raw(),
                src_offset: 0,
                src_length: transfer_buffer.get_size(),
                extent: image_extent,
                dst_image: *dst_image.as_raw(),
                dst_offset: vk::Offset3D::default(),
                dst_length: image_size as vk::DeviceSize,
            })).await?;
        }
        Ok(Arc::new(ImageLoaded {
            handle: dst_image,
            format: Format::new(ElementFormat::U8, 4),
        }))
    }
}
