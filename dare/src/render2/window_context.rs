use std::sync::Arc;
use tokio::sync::RwLock;
use dagal::allocators::Allocator;
use dagal::winit;
use anyhow::Result;
use crate::render2::surface_context::SurfaceContext;

#[derive(Debug)]
pub struct WindowContext {
    pub surface_context: RwLock<Option<SurfaceContext>>,
    pub present_queue: dagal::device::Queue,
    pub window: Arc<RwLock<Option<Arc<winit::window::Window>>>>,
}

#[derive(Debug)]
pub struct WindowContextCreateInfo {
    pub(crate) present_queue: dagal::device::Queue,
    pub(crate) window: Option<Arc<RwLock<Option<Arc<winit::window::Window>>>>>,
}

impl WindowContext {
    pub fn new(ci: WindowContextCreateInfo) -> Self {
        Self {
            surface_context: RwLock::new(None),
            present_queue: ci.present_queue,
            window: ci.window.unwrap_or(Arc::new(RwLock::new(None))),
        }
    }

    pub async fn build_surface(&self, ci: super::surface_context::SurfaceContextCreateInfo<'_>) -> Result<()> {
        let window_guard = self.window.read().await;
        let window = match window_guard.as_ref() {
            None => return Err(anyhow::anyhow!("Window does not exist")),
            Some(w) => w
        };
        *self.surface_context.write().await = Some(
            SurfaceContext::new(
                super::surface_context::InnerSurfaceContextCreateInfo {
                    instance: &ci.instance,
                    physical_device: &ci.physical_device,
                    allocator: ci.allocator,
                    present_queue: self.present_queue.clone(),
                    window: &window,
                    frames_in_flight: ci.frames_in_flight,
                }
            )?
        );
        println!("Built surface");
        Ok(())
    }

    /// Create frames for the window context
    pub async fn create_frames(&self) -> Result<Arc<[super::frame::Frame]>> {
        let sc_guard = self.surface_context.read().await;
        let surface_context = match &*sc_guard {
            None => Err(anyhow::anyhow!("Expected surface context, got None.")),
            Some(sc) => Ok(sc),
        }?;
        let mut frames = Vec::with_capacity(surface_context.frames_in_flight);
        for frame_number in 0..surface_context.frames_in_flight {
            frames.push(super::frame::Frame::new(self, Some(frame_number)).await?);
        }
        Ok(Arc::from(frames.into_boxed_slice()))
    }
}
