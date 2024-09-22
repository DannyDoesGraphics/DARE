use std::sync::Arc;
use dagal::winit;

#[derive(Debug, Clone)]
pub struct Callback(pub(crate) Arc<tokio::sync::Notify>);

#[derive(Debug)]
pub enum RenderServerRequests {
    /// Requests a single frame be rendered
    Render,
    /// Recreates the surface with the given window + sets a new window
    NewWindow(Arc<winit::window::Window>),
    /// Just makes a new surface
    NewSurface,
    /// Stops the server
    Stop,
}

#[derive(Debug)]
pub struct RenderServerPacket {
    pub(super) callback: Callback,
    pub(super) request: RenderServerRequests,
}