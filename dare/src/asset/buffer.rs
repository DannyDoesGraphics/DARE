use crate::asset::asset::{AssetDescriptor, AssetState, AssetUnloaded, MetaDataLocation};
use crate::asset::format::Format;
use crate::asset::manager::AssetError;
use crate::prelude::asset;
use crate::prelude::render;
use anyhow::Result;
use async_stream::stream;
use bytemuck::Pod;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use derivative::Derivative;
use futures::stream::BoxStream;
use futures::{FutureExt, StreamExt};
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::warn;

pub struct Buffer<A: Allocator + 'static> {
    _phantom: PhantomData<A>,
}

impl<A: Allocator + 'static> PartialEq for Buffer<A> {
    fn eq(&self, other: &Self) -> bool {
        true
    }
}

impl<A: Allocator + 'static> AssetDescriptor for Buffer<A> {
    type Loaded = resource::Buffer<A>;
    type Metadata = BufferMetaData<A>;
}

pub struct BufferLoadInfo<A: Allocator + 'static> {
    allocator: ArcAllocator<A>,
    stream_info: BufferStreamInfo,
    transfer: render::util::TransferPool,
    target_format: Option<Format>,
    buffer_location: MemoryLocation,
    usage_flags: vk::BufferUsageFlags,
}

#[derive(Derivative, Clone)]
#[derivative(Debug, Hash, PartialEq)]
pub struct BufferMetaData<A: Allocator + 'static> {
    pub location: MetaDataLocation,
    /// Offset from the buffer
    pub offset: usize,
    /// Describes length from the offset
    pub length: usize,
    /// Stride, if [`None`] then assume stride is [`BufferMetaData::element_size`]
    pub stride: Option<usize>,
    /// Element format
    pub element_format: Format,
    /// Number of elements
    pub element_count: usize,
    #[derivative(Debug = "ignore", Hash = "ignore", PartialEq = "ignore")]
    pub _allocator: PhantomData<A>,
}

fn convert_and_cast<T, U>(slice: Vec<u8>) -> Vec<u8>
where
    T: Pod,
    U: Pod,
    T: Into<U>,
{
    let from_slice: Vec<T> = bytemuck::cast_slice(&slice).to_vec();
    let to_slice: Vec<U> = from_slice.into_iter().map(|x| x.into()).collect();
    bytemuck::cast_slice(&to_slice).to_vec()
}

impl<A: Allocator + 'static> Eq for BufferMetaData<A> {}
impl<A: Allocator + 'static> AssetUnloaded for BufferMetaData<A> {
    type AssetLoaded = resource::Buffer<A>;
    type Chunk = Vec<u8>;
    type StreamInfo = BufferStreamInfo;
    type LoadInfo = BufferLoadInfo<A>;

    /// # Cancellation safety
    /// A buffer stream is considered to not be cancellation safe whatsoever. Cancellation will
    /// lead to UB, primarily, infinitely loading buffers
    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> Result<BoxStream<'static, Result<Self::Chunk>>> {
        let stride = self.stride.unwrap_or(self.element_format.size());
        let element_size = self.element_format.size();
        let element_count: usize = self.element_count;
        let chunk_size = stream_info.chunk_size;
        let mut elements_processed = 0;

        match &self.location {
            MetaDataLocation::Memory(memory) => {
                let memory = memory.clone();
                Ok(Box::pin(stream! {
                    for chunk in memory[self.offset..self.length]
                    .chunks_exact(element_size) {
                        yield Ok(chunk[0..self.element_format.size()].to_owned())
                    }
                }))
            }
            MetaDataLocation::FilePath(path) => {
                let mut file = tokio::fs::File::open(path.clone()).await?;
                file.seek(io::SeekFrom::Start(self.offset as u64)).await?;
                let mut chunk: Vec<u8> = Vec::with_capacity(stream_info.chunk_size);
                Ok(Box::pin(stream! {
                    while elements_processed < element_count {
                        let mut buffer = vec![0; element_size];
                        match file.read_exact(&mut buffer).await {
                            Ok(_) => {
                                if chunk.len() >= chunk_size {
                                    // Round down to 1 element size
                                    yield Ok(chunk.drain(0..((chunk_size / element_size) * element_size)).collect())
                                }
                                chunk.extend_from_slice(&buffer);
                                // Skip the padding
                                if stride - element_size > 0 {
                                    file.seek(io::SeekFrom::Current((stride - element_size) as i64)).await?;
                                }
                                elements_processed += 1;
                            }
                            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                                break;
                            }
                            Err(e) => {
                                yield Err(anyhow::Error::new(e));
                                break;
                            }
                        }
                    }

                    // Dump remaining chunk
                    if !chunk.is_empty() {
                        yield Ok(chunk);
                    }
                }))
            }
            MetaDataLocation::Link(link) => {
                let response = reqwest::get(link).await?;
                let mut buffer: Vec<u8> = Vec::with_capacity(stream_info.chunk_size);
                Ok(Box::pin(stream! {
                    let mut stream = response.bytes_stream();
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                buffer.extend_from_slice(&bytes);

                                while buffer.len() >= stream_info.chunk_size && elements_processed < element_count {
                                    let mut output: Vec<u8> = Vec::with_capacity(element_size);
                                    output.extend_from_slice(&buffer[..element_size]);
                                    yield Ok(buffer.drain(..stream_info.chunk_size).collect());
                                    buffer.drain(..stride);
                                    elements_processed += 1;
                                }

                                if elements_processed >= element_count {
                                    break;
                                }
                            }
                            Err(e) => {
                                yield Err(anyhow::Error::new(e));
                                return;
                            }
                        }
                    }

                    // Yield any remaining bytes in the buffer as the final chunk
                    if !buffer.is_empty() {
                        yield Ok(buffer);
                    }
                }))
            }
        }
    }

    /// Loads a buffer in to dedicated MemoryLocation
    async fn load(
        &self,
        mut load_info: Self::LoadInfo,
        sender: tokio::sync::watch::Sender<Option<Arc<Self::AssetLoaded>>>,
    ) -> Result<Arc<Self::AssetLoaded>> {
        let res: Result<Arc<resource::Buffer<A>>> = {
            let metadata = self.clone();

            let stream = metadata.clone().stream(load_info.stream_info).await?;
            let mut stream = match load_info.target_format {
                None => stream,
                Some(target_format) => {
                    Self::cast_stream(Ok(stream), metadata.element_format, target_format).await?
                }
            };
            let mut write_buffer = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: load_info.allocator.device(),
                allocator: &mut load_info.allocator,
                size: (metadata.element_format.size() * metadata.element_count) as vk::DeviceSize,
                memory_type: load_info.buffer_location,
                usage_flags: load_info.usage_flags,
            })?;
            let mut output_buffer: Option<resource::Buffer<A>> = None;
            match load_info.buffer_location {
                MemoryLocation::GpuToCpu => unimplemented!(), // Wtf!? Why would you ever load from cpu to gpu back to cpu directly????
                MemoryLocation::GpuOnly => {
                    // Swap write buffer to not be written to and make a dedicated transfer buffer
                    let mut transfer_buffer =
                        resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                            device: load_info.allocator.device(),
                            allocator: &mut load_info.allocator,
                            size: load_info.stream_info.chunk_size as vk::DeviceSize,
                            memory_type: load_info.buffer_location,
                            usage_flags: load_info.usage_flags,
                        })?;
                    // swap write with transfer
                    std::mem::swap(&mut transfer_buffer, &mut write_buffer);
                    output_buffer = Some(transfer_buffer);
                }
                MemoryLocation::CpuOnly | MemoryLocation::CpuToGpu => {}
            }
            let mut write_offset: vk::DeviceSize = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                write_buffer.write(write_offset, &chunk)?;
                write_offset += chunk.len() as vk::DeviceSize;
                if load_info.buffer_location == MemoryLocation::GpuOnly {
                    unsafe {
                        load_info
                            .transfer
                            .transfer_gpu(render::util::TransferRequest::Buffer(
                                render::util::BufferTransferRequest {
                                    src_buffer: *write_buffer.as_raw(),
                                    dst_buffer: *output_buffer.as_ref().unwrap().as_raw(),
                                    src_offset: 0,
                                    dst_offset: write_offset,
                                    length: load_info.stream_info.chunk_size as vk::DeviceSize,
                                },
                            ))
                            .await?;
                    }
                }
            }
            let output_buffer: Arc<resource::Buffer<A>> = Arc::new(match load_info.buffer_location {
                MemoryLocation::GpuToCpu => unimplemented!(), // Wtf!? Why would you ever load from cpu to gpu back to cpu directly????
                MemoryLocation::GpuOnly => output_buffer.unwrap(),
                MemoryLocation::CpuOnly | MemoryLocation::CpuToGpu => write_buffer,
            });
            Ok(output_buffer)
        };
        match res {
            Ok(buffer) => {
                sender.send(Some(buffer.clone()))?;
                Ok(buffer)
            }
            Err(e) => {
                sender.send(None)?;
                Err(e)
            }
        }
    }
}

impl<A: Allocator> BufferMetaData<A> {
    /// Loads the entire buffer rather than streaming it in as chunks
    pub async fn load_data(&self) -> Result<Pin<Vec<u8>>> {
        let upper_size = (self.offset + self.length).min(
            self.offset + (self.stride.unwrap_or(self.element_format.size()) * self.element_count),
        );
        match &self.location {
            MetaDataLocation::FilePath(path) => {
                let mut file = tokio::fs::File::open(path).await?;
                file.seek(io::SeekFrom::Start(self.offset as u64)).await?;
                let mut buffer: Vec<u8> = vec![0; self.length];
                let bytes_read = file.read(&mut buffer).await?;
                if bytes_read < self.length {
                    let length = self.length;
                    warn!("File at {:?} has a length smaller than expected from metadata. Expected length: {:?}, got length: {:?}", path, length, bytes_read);
                }
                let _ = buffer.split_off(bytes_read);
                // read only up to element count of data
                let processed_data = buffer[..(upper_size - self.offset)]
                    .chunks_exact(self.stride.unwrap_or(self.element_format.size()))
                    .flat_map(|chunk| chunk[0..self.element_format.size()].to_vec())
                    .collect::<Vec<u8>>();

                Ok(Pin::new(processed_data))
            }
            MetaDataLocation::Memory(memory) => Ok(Pin::new(
                memory[self.offset..upper_size]
                    .chunks_exact(self.stride.unwrap_or(self.element_format.element_size()))
                    .flat_map(|chunk| chunk[0..self.element_format.size()].to_vec())
                    .collect(),
            )),
            MetaDataLocation::Link(link) => {
                unimplemented!()
            }
        }
    }

    pub async fn cast_stream(
        stream: Result<BoxStream<'static, Result<Vec<u8>>>>,
        current_format: Format,
        target_format: Format,
    ) -> Result<BoxStream<'static, Result<Vec<u8>>>> {
        Ok(stream?
            .map(move |chunk| {
                chunk.map(|chunk| {
                    use crate::asset::format::ElementFormat::*;
                    let chunk = match (
                        current_format.element_format(),
                        target_format.element_format(),
                    ) {
                        (U8, U16) => convert_and_cast::<u8, u16>(chunk),
                        (U8, U32) => convert_and_cast::<u8, u32>(chunk),
                        (U8, U64) => convert_and_cast::<u8, u64>(chunk),
                        (U8, I16) => convert_and_cast::<u8, i16>(chunk),
                        (U8, I32) => convert_and_cast::<u8, i32>(chunk),
                        (U8, I64) => convert_and_cast::<u8, i64>(chunk),
                        (U8, F32) => convert_and_cast::<u8, f32>(chunk),
                        (U8, F64) => convert_and_cast::<u8, f64>(chunk),
                        (U16, U32) => convert_and_cast::<u16, u32>(chunk),
                        (U16, U64) => convert_and_cast::<u16, u64>(chunk),
                        (U16, I32) => convert_and_cast::<u16, i32>(chunk),
                        (U16, I64) => convert_and_cast::<u16, i64>(chunk),
                        (U16, F32) => convert_and_cast::<u16, f32>(chunk),
                        (U16, F64) => convert_and_cast::<u16, f64>(chunk),
                        (U32, U64) => convert_and_cast::<u32, u64>(chunk),
                        (U32, I64) => convert_and_cast::<u32, i64>(chunk),
                        (U32, F64) => convert_and_cast::<u32, f64>(chunk),
                        (I8, I16) => convert_and_cast::<i8, i16>(chunk),
                        (I8, I32) => convert_and_cast::<i8, i32>(chunk),
                        (I8, I64) => convert_and_cast::<i8, i64>(chunk),
                        (I8, F32) => convert_and_cast::<i8, f32>(chunk),
                        (I8, F64) => convert_and_cast::<i8, f64>(chunk),
                        (I16, I32) => convert_and_cast::<i16, i32>(chunk),
                        (I16, I64) => convert_and_cast::<i16, i64>(chunk),
                        (I16, F32) => convert_and_cast::<i16, f32>(chunk),
                        (I16, F64) => convert_and_cast::<i16, f64>(chunk),
                        (I32, I64) => convert_and_cast::<i32, i64>(chunk),
                        (I32, F64) => convert_and_cast::<i32, f64>(chunk),
                        (F32, F64) => convert_and_cast::<f32, f64>(chunk),
                        (_, _) => unimplemented!(),
                    };
                    use std::cmp::Ordering;
                    match current_format.dimension().cmp(&target_format.dimension()) {
                        Ordering::Less => chunk
                            .chunks_exact(current_format.dimension() * target_format.element_size())
                            .flat_map(|chunk| {
                                let mut vec = chunk.to_vec();
                                vec.extend_from_slice(&vec![
                                    0u8;
                                    (target_format.dimension()
                                        - current_format.dimension())
                                        * target_format.element_size()
                                ]);
                                vec
                            })
                            .collect::<Vec<u8>>(),
                        Ordering::Equal => chunk,
                        Ordering::Greater => chunk
                            .chunks_exact(current_format.dimension() * target_format.element_size())
                            .flat_map(|chunk| chunk[0..target_format.size()].to_vec())
                            .collect::<Vec<u8>>(),
                    }
                })
            })
            .boxed())
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BufferStreamInfo {
    /// Size of a chunk in bytes
    pub chunk_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::format::{ElementFormat, Format};
    use dagal::allocators::test_allocator::TestAllocator;
    use std::io::Write;

    #[tokio::test]
    async fn test_buffer_metadata_streaming_from_file() {
        let test_file_path = "test_file.txt";
        let mut test_file =
            std::fs::File::create(test_file_path).expect("Failed to create test file");
        writeln!(test_file, "Hello, Rust!").expect("Failed to write to test file");
        let metadata = BufferMetaData::<dagal::allocators::test_allocator::TestAllocator> {
            location: MetaDataLocation::FilePath(String::from(test_file_path).parse().unwrap()),
            offset: 0,
            length: 1024,
            stride: None,
            element_format: Format::new(ElementFormat::U8, 1),
            element_count: "Hello, Rust!".len() + 1,
            _allocator: PhantomData,
        };
        let stream_info = BufferStreamInfo { chunk_size: 4 };
        let mut stream = metadata.stream(stream_info).await.unwrap();
        let expected_chunks = vec![
            "Hell".as_bytes().to_vec(),
            "o, R".as_bytes().to_vec(),
            "ust!".as_bytes().to_vec(),
            "\n".as_bytes().to_vec(),
        ];
        let mut actual_chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => actual_chunks.push(data),
                Err(e) => panic!("Stream returned an error: {:?}", e),
            }
        }
        assert_eq!(actual_chunks, expected_chunks);
        std::fs::remove_file(test_file_path).expect("Failed to delete test file");
    }

    #[tokio::test]
    async fn test_buffer_metadata_streaming_from_file_cast() {
        use std::fs::File;
        let test_file_path = "test_file.bin";

        // Generate a random Vec<u16> and write it to the file
        let random_data: Vec<u16> = vec![12345, 54321, 65535, 0];
        let mut test_file = File::create(test_file_path).expect("Failed to create test file");

        for &value in &random_data {
            test_file
                .write_all(&value.to_le_bytes())
                .expect("Failed to write to test file");
        }

        // Define the metadata for reading the u16 data
        let metadata = BufferMetaData::<dagal::allocators::test_allocator::TestAllocator> {
            location: MetaDataLocation::FilePath(String::from(test_file_path).parse().unwrap()),
            offset: 0,
            length: random_data.len() * std::mem::size_of::<u16>(),
            stride: None,
            element_format: Format::new(ElementFormat::U16, 1),
            element_count: random_data.len(),
            _allocator: PhantomData,
        };

        let stream_info = BufferStreamInfo { chunk_size: 2 };

        let format = metadata.element_format;
        let stream = metadata.stream(stream_info).await;
        let mut stream = BufferMetaData::<TestAllocator>::cast_stream(
            stream,
            format,
            Format::new(ElementFormat::F64, 3),
        )
        .await
        .unwrap();

        let expected_chunks: Vec<Vec<f64>> = vec![
            vec![12345.0, 0.0, 0.0],
            vec![54321.0, 0.0, 0.0],
            vec![65535.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0],
        ];

        let mut actual_chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(data) => {
                    let f64_chunk: Vec<f64> = data
                        .chunks_exact(std::mem::size_of::<f64>())
                        .map(|bytes| f64::from_le_bytes(bytes.try_into().unwrap()))
                        .collect();
                    actual_chunks.push(f64_chunk);
                }
                Err(e) => panic!("Stream returned an error: {:?}", e),
            }
        }

        assert_eq!(actual_chunks, expected_chunks);
        std::fs::remove_file(test_file_path).expect("Failed to delete test file");
    }

    #[tokio::test]
    async fn test_buffer_metadata_load_from_file() {
        let test_file_path = "test_file.txt";
        let mut test_file =
            std::fs::File::create(test_file_path).expect("Failed to create test file");
        writeln!(test_file, "Hello, Rust!").expect("Failed to write to test file");
        let metadata = BufferMetaData::<dagal::allocators::test_allocator::TestAllocator> {
            location: MetaDataLocation::FilePath(String::from(test_file_path).parse().unwrap()),
            offset: 0,
            length: 1024,
            stride: None,
            element_format: Format::new(ElementFormat::U8, 1),
            element_count: "Hello, Rust!".len() + 1,
            _allocator: PhantomData,
        };

        let buffer = metadata
            .load_data()
            .await
            .expect("Failed to load from metadata");
        assert_eq!(
            "Hello, Rust!\n".as_bytes().to_vec(),
            Pin::into_inner(buffer)
        );
        std::fs::remove_file(test_file_path).expect("Failed to delete test file");
    }
}
