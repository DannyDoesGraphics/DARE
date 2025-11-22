use super::super::prelude as asset;
use crate::asset2::loaders::MetaDataStreamable;
use crate::prelude as dare;
use crate::render::util::{ElementFormat, handle_cast_stream};
use bytemuck::Pod;
use bytes::*;
use derivative::Derivative;
use futures::stream::BoxStream;
use futures::{FutureExt, StreamExt, TryStreamExt};

pub struct Buffer {}
impl asset::Asset for Buffer {
    type Metadata = BufferMetaData;
    type Loaded = BufferAsset;
}

#[derive(Debug, PartialEq, Eq)]
pub struct BufferAsset {
    pub data: Box<[u8]>,
    pub length: usize,
    pub format: dare::render::util::Format,
}
impl asset::AssetLoaded for BufferAsset {}

#[derive(Debug, PartialEq, Clone, Derivative)]
#[derivative(Hash)]
pub struct BufferMetaData {
    /// Location of where to find the data
    pub location: asset::MetaDataLocation,
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
unsafe impl Send for BufferMetaData {}
impl Unpin for BufferMetaData {}
impl Eq for BufferMetaData {}

impl MetaDataStreamable for BufferMetaData {
    type Chunk = Bytes;
    type StreamInfo<'a> = BufferStreamInfo;

    async fn stream<'a>(
        &self,
        stream_info: Self::StreamInfo<'a>,
    ) -> anyhow::Result<BoxStream<'a, anyhow::Result<Self::Chunk>>> {
        // we cannot send more than one frame > our expect size
        let chunk_size: usize = stream_info
            .chunk_size
            .min(self.format.size() * self.element_count);
        let mut stream_builder = asset::loaders::StrideStreamBuilder {
            offset: 0, // do not account for offset as we did that prior in file loading
            element_size: self.stored_format.size(),
            element_stride: self.stride.unwrap_or(self.stored_format.size()),
            element_count: self.element_count,
            frame_size: chunk_size,
        };
        match &self.location {
            asset::MetaDataLocation::FilePath(path) => {
                let stream = dare::asset2::loaders::FileStream::from_path(
                    path,
                    self.offset,
                    chunk_size,
                    self.length,
                )
                .await?
                .map_err(|e| anyhow::Error::new(e));
                let stream = stream_builder
                    .build(stream.boxed())
                    .boxed()
                    .map(|res| res.unwrap())
                    .boxed();
                let stream =
                    handle_cast_stream(stream, self.stored_format, self.format, chunk_size).boxed();
                let stream = dare::asset2::loaders::framer::Framer::new(stream, chunk_size)
                    .boxed()
                    .map(|v| anyhow::Ok(v))
                    .boxed();
                Ok(stream)
            }
            asset::MetaDataLocation::Url(link) => {
                let url = reqwest::get(link).await?;
                let stream = url
                    .bytes_stream()
                    .map_err(|e| anyhow::Error::new(e))
                    .boxed();
                stream_builder.offset = self.offset; // account for offset since url has no way to offset
                let stream = stream_builder.build(stream).map(|v| v.unwrap()).boxed();
                let stream =
                    handle_cast_stream(stream, self.stored_format, self.format, chunk_size).boxed();
                let stream = dare::asset2::loaders::framer::Framer::new(stream, chunk_size)
                    .boxed()
                    .map(|v| anyhow::Ok(v))
                    .boxed();
                Ok(stream)
            }
            asset::MetaDataLocation::Memory(memory) => {
                tracing::warn!(
                    "Asset data stored in memory. This is extremely bad and will quickly consume a lot of memory in the system."
                );
                let memory_slice = memory[self.offset..(self.offset + self.length)].to_vec();
                let stream =
                    futures::stream::once(async move { anyhow::Ok(Bytes::from(memory_slice)) })
                        .boxed();
                let stream = stream_builder.build(stream).map(|v| v.unwrap()).boxed();
                let stream =
                    handle_cast_stream(stream, self.stored_format, self.format, chunk_size).boxed();
                let stream = dare::asset2::loaders::framer::Framer::new(stream, chunk_size)
                    .boxed()
                    .map(|v| anyhow::Ok(v))
                    .boxed();
                Ok(stream)
            }
        }
    }
}

impl asset::loaders::MetaDataLoad for BufferMetaData {
    type Loaded = BufferAsset;
    type LoadInfo<'a> = BufferStreamInfo;

    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded> {
        let mut stream = self.stream(load_info).await?;
        let mut data: Vec<u8> = Vec::with_capacity(self.format.size() * self.element_count);
        while let Some(incoming) = stream.next().await {
            let incoming = incoming?;
            data.extend_from_slice(incoming.as_ref());
        }
        let length = data.len();
        Ok(BufferAsset {
            data: data.into_boxed_slice(),
            length,
            format: self.format,
        })
    }
}
impl asset::AssetMetadata for BufferMetaData {}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BufferStreamInfo {
    /// Size of a chunk in bytes
    pub chunk_size: usize,
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::StreamExt;
    use rand::{Rng, RngCore};
    use std::fs;
    use std::path::PathBuf;
    use tokio::io::AsyncWriteExt;

    // Helper function to clean up the file after test
    fn clean_up_file(file_path: &PathBuf) {
        if file_path.exists() {
            fs::remove_file(file_path).unwrap();
        }
    }

    // Helper function to generate a unique file path
    fn generate_unique_file_path(base_name: &str) -> PathBuf {
        let mut rng = rand::rng();
        let random_number: u64 = rng.next_u64();
        let file_name = format!("{}_{}", random_number, base_name);
        let mut file_path = std::env::current_dir().unwrap();
        file_path.push(file_name);
        file_path
    }

    #[tokio::test]
    async fn test_stream_from_file_exact_chunk_size() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_exact_chunk_size.bin");

        // Prepare test data
        let data_size = 1024; // 1KB of data
        let chunk_size = 256; // Chunk size that divides data_size exactly
        let data: Vec<u8> = (0..data_size).map(|x| x as u8).collect();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length: data_size,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: data_size,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the original data
        assert_eq!(streamed_data, data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_non_divisible_chunk_size() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_non_divisible_chunk_size.bin");

        // Prepare test data
        let data_size = 1000; // Data size that isn't divisible by chunk_size
        let chunk_size = 333;
        let data: Vec<u8> = (0..data_size).map(|x| x as u8).collect();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length: data_size,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: data_size,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the original data
        assert_eq!(streamed_data, data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_with_offset() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_with_offset.bin");

        // Prepare test data
        let total_data_size = 2048;
        let offset = 512;
        let length = 1024;
        let chunk_size = 256;
        let data: Vec<u8> = (0..total_data_size).map(|x| x as u8).collect();
        let expected_data = data[offset..offset + length].to_vec();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset,
            length,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: length,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_with_stride() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_with_stride.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 4; // Size of each element
        const STRIDE: usize = 6; // Stride between elements
        let element_count = 100;
        let length = STRIDE * element_count;
        let chunk_size = 60; // Arbitrary chunk size

        // Generate data with STRIDE
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Element data
            data.extend_from_slice(&(i as u32).to_le_bytes());
            // Padding to match the STRIDE
            data.extend_from_slice(&[0u8; STRIDE - ELEMENT_SIZE]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(ELEMENT_SIZE * element_count);
        for i in 0..element_count {
            expected_data.extend_from_slice(&(i as u32).to_le_bytes());
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(STRIDE),
            format: dare::render::util::Format::new(ElementFormat::U32, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U32, 1),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_with_large_data() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_large_data.bin");

        // Prepare large test data
        let data_size = 10 * 1024 * 1024; // 10 MB of data
        let chunk_size = 1024 * 1024; // 1 MB chunk size
        let data: Vec<u8> = (0..data_size).map(|x| x as u8).collect();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length: data_size,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: data_size,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data and verify incrementally
        let mut total_bytes = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            assert_eq!(chunk.len(), chunk_size.min(data_size - total_bytes));
            total_bytes += chunk.len();
        }

        // Verify total bytes read
        assert_eq!(total_bytes, data_size);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_zero_length() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_zero_length.bin");

        // Prepare empty test data
        let data_size = 0;
        let chunk_size = 256;
        let data: Vec<u8> = Vec::new();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length: data_size,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: 0,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            chunks.push(chunk);
        }

        // Verify that no data was streamed
        assert!(chunks.is_empty());

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_from_file_with_partial_final_chunk() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_partial_final_chunk.bin");

        // Prepare test data
        let data_size = 1000; // Data size that will result in a partial final chunk
        let chunk_size = 256;
        let data: Vec<u8> = (0..data_size).map(|x| x as u8).collect();

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length: data_size,
            stride: None,
            format: dare::render::util::Format::new(ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U8, 1),
            element_count: data_size,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect chunks and verify sizes
        let mut chunk_sizes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            chunk_sizes.push(chunk.len());
        }

        // Expected chunk sizes
        let mut expected_sizes = vec![chunk_size; data_size / chunk_size];
        if data_size % chunk_size != 0 {
            expected_sizes.push(data_size % chunk_size);
        }

        // Verify chunk sizes
        assert_eq!(chunk_sizes, expected_sizes);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_f32_vec3_with_stride() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_f32_vec3_with_stride.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 12; // Size of Vec3<f32>
        const STRIDE: usize = 16; // Stride between elements (including padding)
        let element_count = 100;
        let length = STRIDE * element_count;
        let chunk_size = 64; // Arbitrary chunk size

        // Generate data with STRIDE
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Create a Vec3<f32> with values (i as f32, i as f32 + 1.0, i as f32 + 2.0)
            let vec = [i as f32, i as f32 + 1.0, i as f32 + 2.0];
            for &value in &vec {
                data.extend_from_slice(&value.to_le_bytes());
            }
            // Padding to match the STRIDE
            data.extend_from_slice(&[0u8; STRIDE - ELEMENT_SIZE]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(ELEMENT_SIZE * element_count);
        for i in 0..element_count {
            let vec = [i as f32, i as f32 + 1.0, i as f32 + 2.0];
            for &value in &vec {
                expected_data.extend_from_slice(&value.to_le_bytes());
            }
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(STRIDE),
            format: dare::render::util::Format::new(
                ElementFormat::F32,
                3, // Number of components in Vec3<f32>
            ),
            stored_format: dare::render::util::Format::new(
                ElementFormat::F32,
                3, // Number of components in Vec3<f32>
            ),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_f32_vec2_no_stride() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_f32_vec2_no_stride.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 8; // Size of Vec2<f32>
        let element_count = 200;
        let length = ELEMENT_SIZE * element_count;
        let chunk_size = 128; // Arbitrary chunk size

        // Generate contiguous data (no stride)
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Create a Vec2<f32> with values (i as f32, -i as f32)
            let vec = [i as f32, -(i as f32)];
            for &value in &vec {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data is the same as data since there's no stride
        let expected_data = data.clone();

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: None, // No stride
            format: dare::render::util::Format::new(
                ElementFormat::F32,
                2, // Vec2<f32>
            ),
            stored_format: dare::render::util::Format::new(
                ElementFormat::F32,
                2, // Vec2<f32>
            ),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_u16_indices_with_stride() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_u16_indices_with_stride.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 2; // Size of u16
        const STRIDE: usize = 4; // Stride between elements
        let element_count = 50;
        let length = STRIDE * element_count;
        let chunk_size = 50; // Arbitrary chunk size

        // Generate data with STRIDE
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Element data
            let value = i as u16;
            data.extend_from_slice(&value.to_le_bytes());
            // Padding to match the STRIDE
            data.extend_from_slice(&[0u8; STRIDE - ELEMENT_SIZE]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(ELEMENT_SIZE * element_count);
        for i in 0..element_count {
            let value = i as u16;
            expected_data.extend_from_slice(&value.to_le_bytes());
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(STRIDE),
            format: dare::render::util::Format::new(ElementFormat::U16, 1),
            stored_format: dare::render::util::Format::new(ElementFormat::U16, 1),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_f32_mat4_with_stride() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_f32_mat4_with_stride.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 64; // Size of Mat4<f32> (4x4 matrix)
        const STRIDE: usize = 80; // Stride between elements (including padding)
        let element_count = 20;
        let length = STRIDE * element_count;
        let chunk_size = 128; // Arbitrary chunk size

        // Generate data with STRIDE
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Create a Mat4<f32> with incremental values
            for j in 0..16 {
                let value = (i * 16 + j) as f32;
                data.extend_from_slice(&value.to_le_bytes());
            }
            // Padding to match the STRIDE
            data.extend_from_slice(&[0u8; STRIDE - ELEMENT_SIZE]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(ELEMENT_SIZE * element_count);
        for i in 0..element_count {
            for j in 0..16 {
                let value = (i * 16 + j) as f32;
                expected_data.extend_from_slice(&value.to_le_bytes());
            }
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(STRIDE),
            format: dare::render::util::Format::new(
                ElementFormat::F32,
                16, // Mat4<f32> has 16 components
            ),
            stored_format: dare::render::util::Format::new(
                ElementFormat::F32,
                16, // Mat4<f32> has 16 components
            ),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_f32_vec3_with_no_stride_and_offset() -> anyhow::Result<()> {
        // Generate a unique file path
        let file_path = generate_unique_file_path("test_buffer_f32_vec3_no_stride_offset.bin");

        // Prepare test data
        const ELEMENT_SIZE: usize = 12; // Size of Vec3<f32>
        let element_count = 50;
        let total_elements = 100; // Total elements in file
        let offset_elements = 25; // Start reading from element 25
        let length = ELEMENT_SIZE * element_count;
        let offset = ELEMENT_SIZE * offset_elements;
        let chunk_size = 64; // Arbitrary chunk size

        // Generate contiguous data (no stride)
        let mut data = Vec::with_capacity(ELEMENT_SIZE * total_elements);
        for i in 0..total_elements {
            // Create a Vec3<f32> with values (i as f32, i as f32 * 2.0, i as f32 * 3.0)
            let vec = [i as f32, i as f32 * 2.0, i as f32 * 3.0];
            for &value in &vec {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data is from offset to offset + length
        let expected_data = data[offset..offset + length].to_vec();

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset,
            length,
            stride: None, // No stride
            format: dare::render::util::Format::new(
                ElementFormat::F32,
                3, // Vec3<f32>
            ),
            stored_format: dare::render::util::Format::new(
                ElementFormat::F32,
                3, // Vec3<f32>
            ),
            element_count,
            name: "".to_string(),
        };

        // Set up BufferStreamInfo
        let stream_info = BufferStreamInfo { chunk_size };

        // Create the stream
        let mut stream = metadata.stream(stream_info).await?;

        // Collect streamed data
        let mut streamed_data = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            streamed_data.extend_from_slice(&chunk);
        }

        // Verify that the streamed data matches the expected data
        assert_eq!(streamed_data, expected_data);

        // Clean up
        clean_up_file(&file_path);

        Ok(())
    }
}
