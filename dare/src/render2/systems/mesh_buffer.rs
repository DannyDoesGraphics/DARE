use bevy_ecs::prelude as becs;
use std::ops::{Deref, DerefMut};
use std::time::Instant;

/// An array which contains all possible
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, becs::Resource)]
pub struct IndirectIndicesBuffer(pub Vec<u32>);
impl Deref for IndirectIndicesBuffer {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}
impl DerefMut for IndirectIndicesBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut_slice()
    }
}

/// Update all existing surface buffers
pub fn surface_buffer_update_system(mut indirect_buffer: becs::ResMut<IndirectIndicesBuffer>) {}
