use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use dagal::winit;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub enum RenderServerRequest {
    /// Requests a single frame be rendered
    Render,
    /// Surface has been updated
    SurfaceUpdate {
        dimensions: Option<(u32, u32)>,
        raw_handles: Option<dare::window::WindowHandles>,
    },
    /// Stops the manager
    Stop,
}

unsafe impl Send for RenderServerRequest {}
unsafe impl Sync for RenderServerRequest {}

#[derive(Debug)]
pub struct RenderServerPacket {
    pub(super) callback: Option<tokio::sync::oneshot::Sender<()>>,
    pub(super) request: RenderServerRequest,
}

/// Defines deltas to update the render manager with the new relations between different assets
#[derive(Debug)]
pub enum RenderServerAssetRelationDelta {
    Remove(becs::Entity),
}
