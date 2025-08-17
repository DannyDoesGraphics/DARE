use dare_containers as containers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualResource {
    /// Upper 48 bits represent the resource ID, bottom 16 represent resource generation
    data: u64,
    kind: std::any::TypeId
}

impl containers::slot::Slot for VirtualResource {
    fn id(&self) -> u64 {
        self.data >> 16
    }

    fn set_id(&mut self, id: u64) {
        assert!(id <= 0xFFFF_FFFF_FFFF, "ID must be less than 48 bits");
        self.data = (self.data & 0xFFFF) | (id << 16);
    }

    fn new(id: u64) -> Self {
        Self {
            data: (id << 16) | 0xFFFF,
            kind: std::any::TypeId::of::<Self>(),
        }
    }
}

impl containers::slot::SlotWithGeneration for VirtualResource {
    fn generation(&self) -> u64 {
        self.data & 0xFFFF
    }

    fn set_generation(&mut self, generation: u64) {
        assert!(generation <= 0xFFFF, "Generation must be less than 16 bits");
        self.data = (self.data & 0xFFFF_0000_0000) | generation;
    }

    fn new_with_gen(id: u64, generation: u64) -> Self {
        assert!(id <= 0xFFFF_FFFF_FFFF, "ID must be less than 48 bits");
        assert!(generation <= 0xFFFF, "Generation must be less than 16 bits");
        Self {
            data: (id << 16) | generation,
            kind: std::any::TypeId::of::<Self>(),
        }
    }
}