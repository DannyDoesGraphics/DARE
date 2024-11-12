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

#[cfg(all(
    feature = "tokio",
    not(feature = "futures"),
    not(feature = "async-std")
))]
pub(crate) type DEFAULT_LOCKABLE<T> = tokio::sync::Mutex<T>;

#[cfg(all(
    not(feature = "tokio"),
    feature = "futures",
    not(feature = "async-std")
))]
pub(crate) type DEFAULT_LOCKABLE<T> = futures::lock::Mutex<T>;

#[cfg(all(
    not(feature = "tokio"),
    not(feature = "futures"),
    feature = "async-std"
))]
pub(crate) type DEFAULT_LOCKABLE<T> = async_std::sync::Mutex<T>;

#[cfg(not(feature = "concurrent"))]
pub(crate) type DEFAULT_LOCKABLE<T> = std::sync::Mutex<T>;
