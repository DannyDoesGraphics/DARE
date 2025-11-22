use crate::render::contexts::SurfaceContext;
use anyhow::Result;
use bevy_ecs::prelude as becs;

#[derive(Debug, becs::Resource)]
pub struct WindowContext {
    pub present_queue: dagal::device::Queue,
    pub surface_context: Option<SurfaceContext>,
    pub window_handles: crate::window::WindowHandles,
}

#[derive(Debug)]
pub struct WindowContextCreateInfo {
    pub(crate) present_queue: dagal::device::Queue,
    pub(crate) surface: Option<SurfaceContext>,
    pub(crate) window_handles: crate::window::WindowHandles,
}

impl WindowContext {
    pub fn new(ci: WindowContextCreateInfo) -> Self {
        Self {
            surface_context: ci.surface,
            present_queue: ci.present_queue,
            window_handles: ci.window_handles,
        }
    }

    pub fn update_surface(
        &mut self,
        ci: super::surface_context::SurfaceContextUpdateInfo<'_>,
    ) -> Result<()> {
        // remove old
        if let Some(sc) = self.surface_context.take() {
            drop(sc);
        }
        self.surface_context = Some(SurfaceContext::new(
            super::surface_context::InnerSurfaceContextCreateInfo {
                instance: ci.instance,
                surface: None,
                physical_device: ci.physical_device,
                allocator: ci.allocator,
                present_queue: self.present_queue.clone(),
                raw_handles: self.window_handles.clone(),
                extent: ci.dimensions.unwrap_or((800, 600)),
                frames_in_flight: ci.frames_in_flight,
            },
        )?);
        let surface_context = self.surface_context.as_mut().unwrap();
        surface_context.create_frames(&self.present_queue)?;
        Ok(())
    }
}
