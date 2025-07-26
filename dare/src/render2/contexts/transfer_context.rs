use crate::prelude as dare;
use dagal::allocators::GPUAllocatorImpl;
use bevy_ecs::prelude as becs;

/// Context that manages transfer operations and resources
#[derive(Debug, becs::Resource)]
pub struct TransferContext {
    pub transfer_pool: dare::render::util::TransferPool<GPUAllocatorImpl>,
    pub immediate_submit: dare::render::util::ImmediateSubmit,
}

impl TransferContext {
    pub fn new(
        transfer_pool: dare::render::util::TransferPool<GPUAllocatorImpl>,
        immediate_submit: dare::render::util::ImmediateSubmit,
    ) -> Self {
        Self {
            transfer_pool,
            immediate_submit,
        }
    }
}

impl Drop for TransferContext {
    fn drop(&mut self) {
        tracing::trace!("Dropped TransferContext");
    }
}
