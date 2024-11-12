use super::super::prelude as asset;
use crate::asset2::loaders::MetaDataStreamable;
use crate::prelude as dare;
use bytemuck::Pod;
use derivative::Derivative;
use futures::{FutureExt, StreamExt, TryStreamExt};
use std::sync::Arc;

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

#[derive(Debug, Hash, PartialEq, Clone)]
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
}
unsafe impl Send for BufferMetaData {}
impl Unpin for BufferMetaData {}

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
impl Eq for BufferMetaData {}

impl asset::loaders::MetaDataStreamable for BufferMetaData {
    type Chunk = Vec<u8>;
    type StreamInfo<'a> = BufferStreamInfo;

    async fn stream<'a>(
        &self,
        stream_info: Self::StreamInfo<'a>,
    ) -> anyhow::Result<futures_core::stream::BoxStream<'a, anyhow::Result<Self::Chunk>>> {
        let mut stream_builder = asset::loaders::StrideStreamBuilder {
            offset: 0, // do not account for offset as we did that prior in file loading
            element_size: self.format.size(),
            element_stride: self.stride.unwrap_or(self.format.size()),
            element_count: self.element_count,
            frame_size: stream_info.chunk_size,
        };
        match &self.location {
            asset::MetaDataLocation::FilePath(path) => {
                let stream = dare::asset2::loaders::FileStream::from_path(
                    path,
                    self.offset,
                    stream_info.chunk_size,
                    self.length,
                )
                .await?
                .map_err(|e| anyhow::Error::new(e));
                let stream = stream_builder.build(stream.boxed()).boxed();
                Ok(stream)
            }
            asset::MetaDataLocation::Url(link) => {
                let url = reqwest::get(link).await?;
                let stream = url
                    .bytes_stream()
                    .map_err(|e| anyhow::Error::new(e))
                    .boxed();
                stream_builder.offset = self.offset; // account for offset since url has no way to offset
                Ok(stream_builder.build(stream).boxed())
            }
            asset::MetaDataLocation::Memory(memory) => {
                tracing::warn!("Asset data stored in memory. This is extremely bad and will quickly consume a lot of memory in the system.");
                let memory: Arc<[u8]> = memory[self.offset..(self.offset + self.length)]
                    .to_owned()
                    .into();
                let stream = futures::stream::once(async move { anyhow::Ok(memory) }).boxed();
                Ok(stream_builder.build(stream).boxed())
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
        while let Some(mut incoming) = stream.next().await {
            let mut incoming = incoming?;
            data.append(&mut incoming);
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
    use rand::Rng;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::io::AsyncWriteExt;

    // Helper function to clean up the file after test
    fn clean_up_file(file_path: &PathBuf) {
        if file_path.exists() {
            fs::remove_file(file_path).unwrap();
        }
    }

    // Helper function to generate a unique file path
    fn generate_unique_file_path(base_name: &str) -> PathBuf {
        let mut rng = rand::thread_rng();
        let random_number: u64 = rng.gen();
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: data_size,
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: data_size,
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: length,
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
        const element_size: usize = 4; // Size of each element
        const stride: usize = 6; // Stride between elements
        let element_count = 100;
        let length = stride * element_count;
        let chunk_size = 60; // Arbitrary chunk size

        // Generate data with stride
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Element data
            data.extend_from_slice(&(i as u32).to_le_bytes());
            // Padding to match the stride
            data.extend_from_slice(&[0u8; stride - element_size]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(element_size * element_count);
        for i in 0..element_count {
            expected_data.extend_from_slice(&(i as u32).to_le_bytes());
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(stride),
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U32, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count,
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: data_size,
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: 0,
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
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U8, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U8,
                1,
            ),
            element_count: data_size,
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
        const element_size: usize = 12; // Size of Vec3<f32>
        const stride: usize = 16; // Stride between elements (including padding)
        let element_count = 100;
        let length = stride * element_count;
        let chunk_size = 64; // Arbitrary chunk size

        // Generate data with stride
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Create a Vec3<f32> with values (i as f32, i as f32 + 1.0, i as f32 + 2.0)
            let vec = [i as f32, i as f32 + 1.0, i as f32 + 2.0];
            for &value in &vec {
                data.extend_from_slice(&value.to_le_bytes());
            }
            // Padding to match the stride
            data.extend_from_slice(&[0u8; stride - element_size]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(element_size * element_count);
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
            stride: Some(stride),
            format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                3, // Number of components in Vec3<f32>
            ),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                3, // Number of components in Vec3<f32>
            ),
            element_count,
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
        const element_size: usize = 8; // Size of Vec2<f32>
        let element_count = 200;
        let length = element_size * element_count;
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
                dare::render::util::ElementFormat::F32,
                2, // Vec2<f32>
            ),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                2, // Vec2<f32>
            ),
            element_count,
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
        const element_size: usize = 2; // Size of u16
        const stride: usize = 4; // Stride between elements
        let element_count = 50;
        let length = stride * element_count;
        let chunk_size = 50; // Arbitrary chunk size

        // Generate data with stride
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Element data
            let value = i as u16;
            data.extend_from_slice(&value.to_le_bytes());
            // Padding to match the stride
            data.extend_from_slice(&[0u8; stride - element_size]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(element_size * element_count);
        for i in 0..element_count {
            let value = i as u16;
            expected_data.extend_from_slice(&value.to_le_bytes());
        }

        // Set up BufferMetaData
        let metadata = BufferMetaData {
            location: asset::MetaDataLocation::FilePath(file_path.clone()),
            offset: 0,
            length,
            stride: Some(stride),
            format: dare::render::util::Format::new(dare::render::util::ElementFormat::U16, 1),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::U16,
                1,
            ),
            element_count,
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
        const element_size: usize = 64; // Size of Mat4<f32> (4x4 matrix)
        const stride: usize = 80; // Stride between elements (including padding)
        let element_count = 20;
        let length = stride * element_count;
        let chunk_size = 128; // Arbitrary chunk size

        // Generate data with stride
        let mut data = Vec::with_capacity(length);
        for i in 0..element_count {
            // Create a Mat4<f32> with incremental values
            for j in 0..16 {
                let value = (i * 16 + j) as f32;
                data.extend_from_slice(&value.to_le_bytes());
            }
            // Padding to match the stride
            data.extend_from_slice(&[0u8; stride - element_size]);
        }

        // Write test data to file
        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        // Expected data (elements only, without padding)
        let mut expected_data = Vec::with_capacity(element_size * element_count);
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
            stride: Some(stride),
            format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                16, // Mat4<f32> has 16 components
            ),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                16, // Mat4<f32> has 16 components
            ),
            element_count,
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
        const element_size: usize = 12; // Size of Vec3<f32>
        let element_count = 50;
        let total_elements = 100; // Total elements in file
        let offset_elements = 25; // Start reading from element 25
        let length = element_size * element_count;
        let offset = element_size * offset_elements;
        let chunk_size = 64; // Arbitrary chunk size

        // Generate contiguous data (no stride)
        let mut data = Vec::with_capacity(element_size * total_elements);
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
                dare::render::util::ElementFormat::F32,
                3, // Vec3<f32>
            ),
            stored_format: dare::render::util::Format::new(
                dare::render::util::ElementFormat::F32,
                3, // Vec3<f32>
            ),
            element_count,
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
