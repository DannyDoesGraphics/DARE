use bevy_ecs::prelude::*;
use dagal::allocators::Allocator;

use crate::contexts;

pub fn transfer_belt_poll_system<A: Allocator + 'static>(
    mut gpu: NonSendMut<contexts::RenderGpu<A>>,
) {
    let _span = tracy_client::span!("Transfer Belt Poll");
    if let Err(err) = gpu.transfer.poll() {
        tracing::error!(?err, "Transfer belt poll failed");
    }
}

pub fn transfer_belt_flush_system<A: Allocator + 'static>(
    mut gpu: NonSendMut<contexts::RenderGpu<A>>,
) {
    let _span = tracy_client::span!("Transfer Belt Flush");
    if let Err(err) = gpu.transfer.flush() {
        tracing::error!(?err, "Transfer belt flush failed");
    }
}
