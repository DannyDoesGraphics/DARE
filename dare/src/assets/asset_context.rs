use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::descriptor::GPUResourceTable;

/// A context which contains all resources used during rendering
#[derive(Debug)]
pub struct AssetContext<A: Allocator = GPUAllocatorImpl> {
    gpu_rt: GPUResourceTable<A>,
    
}