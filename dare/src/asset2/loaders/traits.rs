use std::fmt::Debug;

/// Defines an asset's streamer s.t. it can be loaded in
///
/// # Cancellation safety
/// It is expected all asset streamable are indeed cancellation safe
pub trait MetaDataStreamable: Unpin + Debug + Send {
    /// Also through as a frame of data
    type Chunk;

    /// Information required to stream
    type StreamInfo<'a>: 'a
    where
        Self: 'a;

    /// Streams an asset in
    ///
    /// # Send safety
    /// We do not support send safety as you should not be sending large number of bytes
    /// across threads and keep them as local (to the thread) as possible to maximize cache
    /// efficiency
    async fn stream<'a>(
        &self,
        stream_info: Self::StreamInfo<'a>,
    ) -> anyhow::Result<futures::stream::BoxStream<'a, anyhow::Result<Self::Chunk>>>;
}

/// Defines an asset s.t. it automatically handles the streaming + loading
pub trait MetaDataLoad: Unpin + Debug + Send {
    /// Type of asset when loaded
    type Loaded: Send;

    /// Load information
    type LoadInfo<'a>: Send
    where
        Self: 'a;

    /// Loads an asset in
    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded>;
}
