use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
pub use bevy_ecs::prelude as becs;

#[derive(Clone, becs::Resource)]
pub struct FrameCount(pub(crate) Arc<AtomicUsize>);

impl Default for FrameCount {
    fn default() -> Self {
        Self (Arc::new(AtomicUsize::new(0)))
    }
}