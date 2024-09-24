use dagal::winit;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Callback(pub(crate) Arc<tokio::sync::Notify>);

#[derive(Debug)]
pub enum RenderServerRequests {
    /// Requests a single frame be rendered
    Render,
    /// Stops the server
    Stop,
}

#[derive(Debug)]
pub struct RenderServerPacket {
    pub(super) callback: Callback,
    pub(super) request: RenderServerRequests,
}
