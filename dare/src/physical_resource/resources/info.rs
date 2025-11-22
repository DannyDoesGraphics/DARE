use dagal::allocators::GPUAllocatorImpl;
use derivative::Derivative;

/// Used to represent the maximum size a chunk can be sent through a stream
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StreamInfo {
    pub chunk_size: usize,
}

/// Used to provide basic information to initialize streams
#[derive(Debug, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct GPUStreamInfo {
    pub stream_info: StreamInfo,
    #[derivative(PartialEq = "ignore")]
    pub transfer: crate::render::util::transfer::TransferPool<GPUAllocatorImpl>,
    #[derivative(PartialEq = "ignore")]
    pub allocator: GPUAllocatorImpl,
    #[derivative(PartialEq = "ignore")]
    pub buffer:
        std::sync::Arc<tokio::sync::Mutex<Option<dagal::resource::Buffer<GPUAllocatorImpl>>>>,
}
