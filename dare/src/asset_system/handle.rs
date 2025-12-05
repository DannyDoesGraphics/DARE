use bevy_ecs::prelude::*;

/// A handle to a mesh asset
///
/// The handle is a 64-bit value where the lower 32 bits represent the asset ID
/// and the upper 32 bits represent the generation of the asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Component)]
pub struct MeshHandle {
    id: u64,
}

impl dare_containers::slot::Slot for MeshHandle {
    /// Get the unique identifier for this handle (bottom 32 bits)
    fn id(&self) -> u64 {
        self.id & 0xFFFFFFFF
    }

    fn set_id(&mut self, id: u64) {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        self.id = (self.id & 0xFFFFFFFF00000000) | (id & 0xFFFFFFFF);
    }

    fn new(id: u64) -> Self {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        MeshHandle { id }
    }
}

impl dare_containers::slot::SlotWithGeneration for MeshHandle {
    fn generation(&self) -> u64 {
        self.id >> 32
    }

    fn set_generation(&mut self, generation: u64) {
        assert!(
            generation <= 0xFFFFFFFF,
            "Generation must fit within 32 bits"
        );
        self.id = (self.id & 0x00000000FFFFFFFF) | (generation << 32);
    }

    fn new_with_gen(id: u64, generation: u64) -> Self {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        assert!(
            generation <= 0xFFFFFFFF,
            "Generation must fit within 32 bits"
        );
        MeshHandle {
            id: (generation << 32) | (id & 0xFFFFFFFF),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Component)]
pub struct GeometryHandle {
    id: u64,
}

impl dare_containers::slot::Slot for GeometryHandle {
    /// Get the unique identifier for this handle (bottom 32 bits)
    fn id(&self) -> u64 {
        self.id & 0xFFFFFFFF
    }

    fn set_id(&mut self, id: u64) {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        self.id = (self.id & 0xFFFFFFFF00000000) | (id & 0xFFFFFFFF);
    }

    fn new(id: u64) -> Self {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        GeometryHandle { id }
    }
}

impl dare_containers::slot::SlotWithGeneration for GeometryHandle {
    fn generation(&self) -> u64 {
        self.id >> 32
    }

    fn set_generation(&mut self, generation: u64) {
        assert!(
            generation <= 0xFFFFFFFF,
            "Generation must fit within 32 bits"
        );
        self.id = (self.id & 0x00000000FFFFFFFF) | (generation << 32);
    }

    fn new_with_gen(id: u64, generation: u64) -> Self {
        assert!(id <= 0xFFFFFFFF, "ID must fit within 32 bits");
        assert!(
            generation <= 0xFFFFFFFF,
            "Generation must fit within 32 bits"
        );
        GeometryHandle {
            id: (generation << 32) | (id & 0xFFFFFFFF),
        }
    }
}
