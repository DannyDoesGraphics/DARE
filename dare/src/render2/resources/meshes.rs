use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dare_containers::prelude as containers;
use std::ops::{Deref, DerefMut};

#[derive(Debug, becs::Resource)]
pub struct MeshesContainers {
    containers: containers::InsertionSortSlotMap<dare::engine::components::Surface>,
}

impl MeshesContainers {
    fn new() -> Self {
        Self {
            containers: Default::default(),
        }
    }
}

impl Deref for MeshesContainers {
    type Target = containers::InsertionSortSlotMap<dare::engine::components::Surface>;

    fn deref(&self) -> &Self::Target {
        &self.containers
    }
}

impl DerefMut for MeshesContainers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.containers
    }
}
