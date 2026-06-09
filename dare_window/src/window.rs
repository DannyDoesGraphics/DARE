use std::ops::Deref;
use std::sync::Arc;

pub use bevy_ecs::prelude::*;
use dagal::raw_window_handle;
use dagal::winit;

use crate::WindowHandles;

#[derive(Debug, Clone, Resource, Default, PartialEq, Eq)]
pub enum Window {
    #[default]
    None,
    Window {
        raw_window_handle: raw_window_handle::RawWindowHandle,
        raw_display_handle: raw_window_handle::RawDisplayHandle,
        /// (width, height)
        physical_size: (u32, u32),
        size_changed: bool,
    },
}
unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    pub fn handles(&self) -> WindowHandles {
        let Window::Window {
            raw_window_handle,
            raw_display_handle,
            ..
        } = self
        else {
            panic!("window handles requested while Window::None");
        };
        WindowHandles {
            raw_window_handle: Arc::new(*raw_window_handle),
            raw_display_handle: Arc::new(*raw_display_handle),
        }
    }

    pub fn take_size_changed(&mut self) -> bool {
        match self {
            Window::Window { size_changed, .. } => std::mem::take(size_changed),
            Window::None => false,
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Window::None => false,
            Window::Window { physical_size, .. } => physical_size.0 > 0 && physical_size.1 > 0,
        }
    }

    pub fn same_surface(&self, other: &Self) -> bool {
        match (self, other) {
            (Window::None, Window::None) => true,
            (
                Window::Window {
                    raw_window_handle: a_wh,
                    raw_display_handle: a_dh,
                    physical_size: a_size,
                    ..
                },
                Window::Window {
                    raw_window_handle: b_wh,
                    raw_display_handle: b_dh,
                    physical_size: b_size,
                    ..
                },
            ) => a_wh == b_wh && a_dh == b_dh && a_size == b_size,
            _ => false,
        }
    }
}

#[derive(Debug, Resource)]
pub struct WinitWindow(pub Arc<winit::window::Window>);
impl Deref for WinitWindow {
    type Target = winit::window::Window;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
