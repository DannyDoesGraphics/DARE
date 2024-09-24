pub use bevy_ecs::prelude as becs;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[derive(Clone, becs::Resource)]
pub struct FrameCount(pub(crate) Arc<AtomicUsize>);

impl Default for FrameCount {
    fn default() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }
}
