pub use bevy_ecs::prelude as becs;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[derive(Clone, becs::Resource)]
pub struct FrameCount(pub(crate) Arc<AtomicUsize>);

impl Default for FrameCount {
    fn default() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }
}

impl Deref for FrameCount {
    type Target = Arc<AtomicUsize>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FrameCount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
