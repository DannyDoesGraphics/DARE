use futures::StreamExt;
use futures::stream::{BoxStream, Stream};
use std::fmt::Debug;
use std::hash::Hash;

/// Metadata for a resource
pub trait ResourceMetadata: 'static + Sized + Hash + Clone + Send + Sync {
    /// Type of asset when loaded
    type Loaded: Send;

    /// Information to start a stream.
    type CPUStreamInfo<'a>: 'a
    where
        Self: 'a;

    /// Load information
    type LoadInfo<'a>: Send
    where
        Self: 'a;

    /// Return true if this metadata supports streaming under current conditions.
    ///
    /// Default: false.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Loads an asset in
    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded>;

    /// Streamed chunk type when streaming is used.
    type Chunk;

    /// If streaming is supported, produce a stream of chunks.
    ///
    /// Default: an empty stream, suitable when [`Self::supports_streaming`] is false.
    async fn stream<'a>(
        &self,
        stream_info: Self::CPUStreamInfo<'a>,
    ) -> anyhow::Result<futures::stream::LocalBoxStream<'a, anyhow::Result<Self::Chunk>>> {
        Ok(futures::stream::empty().boxed_local())
    }
}

/// Defines the base loaded resource
pub trait Resource: 'static {
    /// Resource metadata
    type Metadata: ResourceMetadata;
    /// Represents a cpu sided loaded resource
    type CPULoaded: 'static + Debug + PartialEq + Eq;
}

/// Defines a resource which can support GPU direct loading
pub trait GPUUploadable: ResourceMetadata {
    /// Opaque GPU resource handle
    type GPULoaded: Send;

    /// Information to start a GPU upload stream.
    type GPUStreamInfo<'a>: 'a
    where
        Self: 'a;

    type GPULoadInfo<'a>: 'a
    where
        Self: 'a;

    /// Expected chunk from a gpu stream
    type GPUChunk<'a>: 'a
    where
        Self: 'a;

    /// Returns true if the resource supports GPU streaming.
    ///
    /// Default: false
    fn supports_gpu_streaming(&self) -> bool {
        false
    }

    /// Initiate a gpu stream
    ///
    /// Default: empty stream
    async fn gpu_stream<'a>(
        &self,
        stream_info: Self::GPUStreamInfo<'a>,
    ) -> anyhow::Result<futures::stream::LocalBoxStream<'a, anyhow::Result<Self::GPUChunk<'a>>>>
    {
        Ok(futures::stream::empty().boxed_local())
    }

    /// Upload a resource directly onto GPU
    async fn upload_gpu(
        &self,
        upload_info: Self::GPULoadInfo<'_>,
    ) -> anyhow::Result<Self::GPULoaded>;
}
