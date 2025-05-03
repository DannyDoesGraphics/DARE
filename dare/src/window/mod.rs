pub mod input;
pub mod prelude;

use dagal::raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct WindowHandles {
    pub raw_window_handle: Arc<RawWindowHandle>,
    pub raw_display_handle: Arc<RawDisplayHandle>,
}

unsafe impl Send for WindowHandles {}
unsafe impl Sync for WindowHandles {}
