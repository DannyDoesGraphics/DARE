use std::{hash::Hash, marker::PhantomData};

use bevy_ecs::prelude::*;
use dare_containers::slot::{Slot, SlotWithGeneration};

use crate::Asset;

/// [`AssetHandle<T>`] but is type erased
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct ErasedAssetHandle {
    id: u128,
    ty: std::any::TypeId,
}

impl ErasedAssetHandle {
    pub fn new<T: Asset + 'static>(handle: &AssetHandle<T>) -> Self {
        Self {
            id: handle.id,
            ty: std::any::TypeId::of::<T>(),
        }
    }
}

#[derive(Debug, Copy, Component)]
pub struct AssetHandle<A: Asset> {
    /// Id belongs in [0, 63] and generation in [64, 127]
    id: u128,
    _ty: PhantomData<A>,
}
impl<A: Asset> PartialEq for AssetHandle<A> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<A: Asset> Hash for AssetHandle<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl<A: Asset> Clone for AssetHandle<A> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _ty: self._ty,
        }
    }
}
impl<A: Asset> Eq for AssetHandle<A> {}

impl<A: Asset> Slot for AssetHandle<A> {
    fn set_id(&mut self, id: u64) {
        self.id = (self.id >> 64 << 64) | id as u128;
    }

    fn id(&self) -> u64 {
        self.id as u64
    }

    fn new(id: u64) -> Self {
        Self {
            id: id as u128,
            _ty: PhantomData,
        }
    }
}
impl<A: Asset> SlotWithGeneration for AssetHandle<A> {
    fn new_with_gen(id: u64, generation: u64) -> Self {
        Self {
            id: (id as u128) | (generation as u128) << 64,
            _ty: PhantomData,
        }
    }

    fn generation(&self) -> u64 {
        (self.id >> 64) as u64
    }

    fn set_generation(&mut self, generation: u64) {
        self.id = self.id as u64 as u128 | (generation as u128) << 64;
    }
}

/// A handle to a [`crate::MeshAsset`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Component)]
pub struct MeshHandle {
    id: u64,
}
impl dare_containers::slot::Slot for MeshHandle {
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
