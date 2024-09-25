/// Defines generic traits to abstract over various async libraries
pub mod lockable;
pub mod lockable_impl;

#[cfg(feature = "tokio")]
pub use tokio;
/// Redefines
#[cfg(feature = "winit")]
pub use winit;

pub use ash;
pub use raw_window_handle;

#[cfg(feature = "tokio")]
pub(crate) type DEFAULT_LOCKABLE<T> = tokio::sync::Mutex<T>;

#[cfg(not(feature = "tokio"))]
pub(crate) type DEFAULT_LOCKABLE<T> = std::sync::Mutex<T>;
