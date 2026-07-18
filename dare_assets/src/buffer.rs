use dagal::allocators::GPUAllocatorImpl;

use crate::{DataLocation, Format};

#[derive(Debug, Clone)]
pub struct Buffer {
    pub location: DataLocation,
    pub format: Format,
    pub stride: Option<u64>,
    pub count: u64,
}

impl crate::Asset for Buffer {
    type GpuResource = dagal::resource::Buffer<GPUAllocatorImpl>;
}
