use bevy_ecs::prelude::*;

use crate::{Geometry, GeometryHandle, MeshAsset, MeshHandle};

/// Asset manager is responsible for handling high-level asset operations.
#[derive(Debug, Resource, Default)]
pub struct AssetManager {
    pub geometry_store: dare_containers::slot_map::SlotMap<Geometry, GeometryHandle>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }
}
