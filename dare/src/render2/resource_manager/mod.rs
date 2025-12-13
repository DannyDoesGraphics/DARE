mod handle;

pub use handle::*;

use bevy_ecs::prelude::*;
use dare_assets::*;
use std::collections::HashMap;

pub enum ResourceCommand {
    CreateMesh {},
}

/// Resource manager is responsible for mapping and tracking GPU resources such as textures and buffers
#[derive(Debug)]
pub struct ResourceManager {}

/// ErasedStore is a hashmap that stores boxed trait objects
struct ErasedStore {
    store: HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

/// Map asset handles to resource handles
#[derive(Debug, Resource)]
pub struct AssetManagerToResourceManager {
    pub resource_map: HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
    pub geometries: HashMap<GeometryHandle, ResourceHandle>,
}
