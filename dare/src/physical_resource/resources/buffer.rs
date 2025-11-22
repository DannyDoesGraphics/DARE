use super::ByteChunk;
use bevy_tasks::BoxedFuture;
use bytes::Bytes;
use dagal::{ash::vk, resource::traits::Resource};
use dare_containers::prelude::UniqueSlotMap;
use derivative::Derivative;
use futures::{
    FutureExt, StreamExt, TryStreamExt,
    future::LocalBoxFuture,
    stream::{BoxStream, LocalBoxStream},
};
use tokio::io::AsyncSeekExt;
use tokio_util::io::ReaderStream;

use super::super::traits;
use crate::{
    asset::loaders::StrideStreamBuilder, physical_resource::ResourceMetadata, prelude as dare,
};

#[derive(Derivative, PartialEq, Eq, Clone, Debug)]
#[derivative(Hash)]
pub struct BufferMetadata {
    pub location: super::super::ResourceLocation,
    /// Offset from the buffer
    pub offset: usize,
    /// Describes length from the offset
    pub length: usize,
    /// Stride, if [`None`] then assume stride is [`BufferMetaData::element_size`]
    pub stride: Option<usize>,
    /// Target Format
    pub format: dare::render::util::Format,
    /// Stored format
    pub stored_format: dare::render::util::Format,
    /// Number of elements
    pub element_count: usize,
    /// Name of the buffer
    #[derivative(Hash = "ignore")]
    pub name: String,
}

/// Giant array of bytes
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CPUBuffer {
    pub data: Bytes,
    pub format: dare::render::util::Format,
}

impl traits::ResourceMetadata for BufferMetadata {
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn stream<'a>(
        &self,
        stream_info: Self::CPUStreamInfo<'a>,
    ) -> anyhow::Result<futures::stream::LocalBoxStream<'a, anyhow::Result<Self::Chunk>>> {
        let asset_stream: LocalBoxStream<anyhow::Result<Bytes>> = match &self.location {
            super::super::ResourceLocation::FilePath(path) => {
                let mut file = tokio::fs::File::open(path).await?;
                if self.offset > 0 {
                    file.seek(std::io::SeekFrom::Start(self.offset as u64))
                        .await?;
                }
                let rs = ReaderStream::with_capacity(file, stream_info.chunk_size);
                rs.map(|r| r.map_err(|_| anyhow::Error::msg("Failed to read bytes from file")))
                    .boxed_local()
            }
            super::super::ResourceLocation::URL(url) => {
                let req = if self.offset == 0 {
                    reqwest::get(url).await?
                } else {
                    let client = reqwest::Client::new();
                    client
                        .get(url)
                        .header(reqwest::header::RANGE, format!("bytes={}-", self.offset))
                        .send()
                        .await?
                        .error_for_status()?
                };
                let stream = req
                    .bytes_stream()
                    .map(|r| r.map_err(|_| anyhow::Error::msg("Failed to read bytes from URL")))
                    .boxed_local();
                stream
            }
            super::super::ResourceLocation::Memory(memory) => {
                let bytes = Bytes::copy_from_slice(&memory[self.offset..]);
                futures::stream::once(async move { Ok(bytes) }).boxed_local()
            }
            _ => unimplemented!(),
        };
        // Create strided stream that handles offset/stride logic internally
        let stream_builder = StrideStreamBuilder {
            offset: 0,
            element_size: self.format.size(),
            element_stride: self.stride.unwrap_or(self.format.size()),
            element_count: self.element_count,
            frame_size: stream_info.chunk_size,
        };
        let strided_stream = stream_builder.build(asset_stream).boxed_local();

        // Use scan to accumulate byte offset across chunks
        let stream_with_offsets = strided_stream.scan(0usize, |byte_offset, chunk| {
            let chunk = match chunk {
                Ok(data) => data,
                Err(e) => return futures::future::ready(Some(Err(e))),
            };

            let current_offset = *byte_offset;
            *byte_offset += chunk.len();

            futures::future::ready(Some(Ok(ByteChunk {
                data: chunk,
                offset: current_offset,
            })))
        });

        Ok(stream_with_offsets.boxed_local())
    }

    type Loaded = CPUBuffer;

    type CPUStreamInfo<'a> = super::info::StreamInfo;

    type LoadInfo<'a> = super::info::StreamInfo;

    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded> {
        // We don't need it in this case.
        unimplemented!()
    }

    type Chunk = ByteChunk;
}

impl traits::GPUUploadable for BufferMetadata {
    type GPULoaded = ();

    type GPUStreamInfo<'a> = super::info::GPUStreamInfo;

    type GPULoadInfo<'a> = super::info::StreamInfo;

    type GPUChunk<'a> = LocalBoxFuture<'a, anyhow::Result<()>>;

    fn supports_gpu_streaming(&self) -> bool {
        true
    }

    async fn gpu_stream<'a>(
        &self,
        stream_info: Self::GPUStreamInfo<'a>,
    ) -> anyhow::Result<futures::stream::LocalBoxStream<'a, anyhow::Result<Self::GPUChunk<'a>>>>
    {
        let cpu_stream = self.stream(stream_info.stream_info).await?;
        let transfer = stream_info.transfer.clone();
        let allocator = stream_info.allocator.clone();
        let dst_buffer = stream_info.buffer.clone();
        let name = self.name.clone();

        let stream = cpu_stream
            .map(
                move |r| -> anyhow::Result<LocalBoxFuture<'a, anyhow::Result<()>>> {
                    let chunk = r?;
                    let transfer = transfer.clone();
                    let allocator = allocator.clone();
                    let dst_buffer = dst_buffer.clone();
                    let name = name.clone();
                    let fut = async move {
                        let mut allocator = allocator;
                        let transfer = transfer.clone();
                        let mut staging_buffer = dagal::resource::Buffer::new(
                            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                                device: transfer.get_device(),
                                name: Some(format!("{}_staging", name)),
                                allocator: &mut allocator,
                                size: chunk.data.len() as u64,
                                memory_type: dagal::allocators::MemoryLocation::CpuToGpu,
                                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                                    | vk::BufferUsageFlags::TRANSFER_DST
                                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                                    | vk::BufferUsageFlags::STORAGE_BUFFER,
                            },
                        )?;
                        staging_buffer.write(0, &chunk.data)?;
                        let mut dst_buffer_guard = dst_buffer.lock().await;
                        if let Some(dst_buf) = dst_buffer_guard.take() {
                            let (staging_returned, dst_returned) = transfer
                                .buffer_to_buffer_transfer(
                                    crate::render::util::transfer::TransferBufferToBuffer {
                                        src_buffer: staging_buffer,
                                        dst_buffer: dst_buf,
                                        src_offset: 0,
                                        dst_offset: chunk.offset as u64,
                                        length: chunk.data.len() as u64,
                                    },
                                )
                                .await?;
                            *dst_buffer_guard = Some(dst_returned);
                            drop(staging_returned);
                        } else {
                            return Err(anyhow::Error::msg("Destination buffer not available"));
                        }

                        Ok::<(), anyhow::Error>(())
                    }
                    .boxed_local();
                    Ok(fut)
                },
            )
            .boxed_local();
        Ok(stream)
    }

    async fn upload_gpu(
        &self,
        upload_info: Self::GPULoadInfo<'_>,
    ) -> anyhow::Result<Self::GPULoaded> {
        // We don't need it in this case.
        unimplemented!()
    }
}

impl traits::Resource for BufferMetadata {
    type Metadata = Self;

    type CPULoaded = CPUBuffer;
}
