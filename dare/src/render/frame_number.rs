pub use bevy_ecs::prelude as becs;

#[derive(Clone, becs::Resource)]
pub struct FrameCounter(pub(crate) usize);

impl Default for FrameCounter {
    fn default() -> Self {
        Self(0)
    }
}

impl FrameCounter {
    pub fn get(&self) -> usize {
        self.0
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }
}
