use crate::render2::contexts::SurfaceContext;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use std::sync::RwLock;

#[derive(Debug, becs::Resource)]
pub struct WindowContext {
    pub present_queue: dagal::device::Queue,
    pub surface_context: RwLock<Option<SurfaceContext>>,
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
            surface_context: RwLock::new(ci.surface),
            present_queue: ci.present_queue,
            window_handles: ci.window_handles,
        }
    }

    pub fn update_surface(
        &self,
        ci: super::surface_context::SurfaceContextUpdateInfo<'_>,
    ) -> Result<()> {
        // remove old
        if let Some(sc) = self.surface_context.write().unwrap().take() {
            drop(sc);
        }
        let mut surface_guard = self.surface_context.write().unwrap();
        *surface_guard = Some(SurfaceContext::new(
            super::surface_context::InnerSurfaceContextCreateInfo {
                instance: ci.instance,
                surface: None,
                physical_device: ci.physical_device,
                allocator: ci.allocator,
                present_queue: self.present_queue.clone(),
                raw_handles: self.window_handles.clone(),
                extent: (0, 0),
                frames_in_flight: ci.frames_in_flight,
            },
        )?);
        let surface_context = surface_guard.as_mut().unwrap();
        surface_context.create_frames(&self.present_queue)?;
        Ok(())
    }
}
