use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use dagal::winit;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Callback(pub(crate) Arc<tokio::sync::Notify>);

#[derive(Debug)]
pub enum RenderServerNoCallbackRequest {
    /// Requests a single frame be rendered
    Render,
    /// Stops the manager
    Stop,
}
#[derive(Debug)]
pub enum InnerRenderServerRequest {
    Delta(RenderServerAssetRelationDelta),
}

#[derive(Debug)]
pub struct RenderServerPacket {
    pub(super) callback: Callback,
    pub(super) request: RenderServerNoCallbackRequest,
}

/// Defines deltas to update the render manager with the new relations between different assets
#[derive(Debug)]
pub enum RenderServerAssetRelationDelta {
    Remove(becs::Entity),
}
