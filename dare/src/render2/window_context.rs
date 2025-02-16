use crate::render2::surface_context::SurfaceContext;
use anyhow::Result;
use dagal::allocators::Allocator;
use dagal::raw_window_handle::HasRawDisplayHandle;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug)]
pub struct WindowContext {
    pub present_queue: dagal::device::Queue,
    pub surface_context: RwLock<Option<SurfaceContext>>,
}

#[derive(Debug)]
pub struct WindowContextCreateInfo {
    pub(crate) present_queue: dagal::device::Queue,
}

impl WindowContext {
    pub fn new(ci: WindowContextCreateInfo) -> Self {
        Self {
            surface_context: RwLock::new(None),
            present_queue: ci.present_queue,
        }
    }

    pub fn update_surface(
        &self,
        ci: super::surface_context::SurfaceContextUpdateInfo<'_>,
    ) -> Result<()> {
        if let Some(sc) = self.surface_context.write().unwrap().take() {
            drop(sc);
        }
        unsafe {
            let mut surface_guard = self.surface_context.write().unwrap();
            *surface_guard = Some(SurfaceContext::new(
                super::surface_context::InnerSurfaceContextCreateInfo {
                    instance: &ci.instance,
                    physical_device: &ci.physical_device,
                    allocator: ci.allocator,
                    present_queue: self.present_queue.clone(),
                    window: ci.window,
                    frames_in_flight: ci.frames_in_flight,
                },
            )?);
            let surface_context = surface_guard.as_mut().unwrap();
            surface_context.create_frames(&self.present_queue)?;
        }
        Ok(())
    }
}
