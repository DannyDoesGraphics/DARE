use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// Describes a window we can interface with
pub trait DagalWindow: WindowDimensions + HasWindowHandle + HasDisplayHandle + Sized {}

pub trait WindowDimensions {
    /// Window width
    fn width(&self) -> u32;
    /// Window height
    fn height(&self) -> u32;
}

#[cfg(feature = "winit")]
impl WindowDimensions for winit::window::Window {
    fn width(&self) -> u32 {
        self.inner_size().to_logical(self.scale_factor()).width
    }

    fn height(&self) -> u32 {
        self.inner_size().to_logical(self.scale_factor()).height
    }
}
#[cfg(feature = "winit")]
impl DagalWindow for winit::window::Window {}
