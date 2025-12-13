use bevy_ecs::prelude::*;
use dare_containers::slot::*;

/// A "virtual" resource handle
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Resource)]
pub struct ResourceHandle {
    id: u64,
    generation: u64,
}

impl Slot for ResourceHandle {
    fn new(id: u64) -> Self {
        Self { id, generation: 0 }
    }

    fn set_id(&mut self, id: u64) {
        self.id = id;
    }

    fn id(&self) -> u64 {
        self.id
    }
}

impl SlotWithGeneration for ResourceHandle {
    fn new_with_gen(id: u64, generation: u64) -> Self {
        Self { id, generation }
    }

    fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    fn generation(&self) -> u64 {
        self.generation
    }
}
