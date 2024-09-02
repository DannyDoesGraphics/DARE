use crate::asset;
use crate::asset::asset::{AssetDescriptor, AssetUnloaded, MetaDataLocation};
use crate::asset::format::Format;
use crate::asset::manager::AssetError;
use anyhow::Result;
use async_stream::stream;
use bytemuck::Pod;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::resource;
use derivative::Derivative;
use futures::stream::BoxStream;
use futures::{FutureExt, StreamExt};
use gltf::accessor::DataType;
use std::io;
use std::iter::Map;
use std::marker::PhantomData;
use std::pin::Pin;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::warn;

pub struct Buffer<A: Allocator> {
    _phantom: PhantomData<A>,
}

impl<A: Allocator> PartialEq for Buffer<A> {
    fn eq(&self, other: &Self) -> bool {
        true
    }
}

impl<A: Allocator> AssetDescriptor for Buffer<A> {
    type Loaded = resource::Buffer<A>;
    type Metadata = BufferMetaData<A>;
}

#[derive(Derivative, Clone)]
#[derivative(Debug, Hash, PartialEq)]
pub struct BufferMetaData<A: Allocator> {
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

impl<A: Allocator> Eq for BufferMetaData<A> {}
impl<A: Allocator> AssetUnloaded for BufferMetaData<A> {
    type AssetLoaded = resource::Buffer<A>;
    type Chunk = Vec<u8>;
    type StreamInfo = BufferLoadInfo;

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
                                    /// Round down to 1 element size
                                    yield Ok(chunk.drain(0..((chunk_size / element_size) * element_size)).collect())
                                }
                                chunk.extend_from_slice(&buffer);
                                file.seek(io::SeekFrom::Current((stride - element_size) as i64)).await?;
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
}

impl<A: Allocator> BufferMetaData<A> {
    /// Loads the entire buffer rather than streaming it in as chunks
    pub async fn load(&self) -> Result<Pin<Vec<u8>>> {
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
                println!("{:?}", buffer);
                let processed_data = buffer
                    .chunks_exact(self.stride.unwrap_or(self.element_format.size()))
                    .flat_map(|chunk| chunk[0..self.element_format.size()].to_vec())
                    .collect::<Vec<u8>>();

                Ok(Pin::new(processed_data))
            }
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
                    use asset::format::ElementFormat::*;
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
pub struct BufferLoadInfo {
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
        let stream_info = BufferLoadInfo { chunk_size: 4 };
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

        let stream_info = BufferLoadInfo { chunk_size: 2 };

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

        let buffer = metadata.load().await.expect("Failed to load from metadata");
        assert_eq!(
            "Hello, Rust!\n".as_bytes().to_vec(),
            Pin::into_inner(buffer)
        );
        std::fs::remove_file(test_file_path).expect("Failed to delete test file");
    }
}
