mod handle;

use dagal::allocators::GPUAllocatorImpl;
pub use handle::*;

use bevy_ecs::prelude::*;
use dare_assets::*;
use std::collections::HashMap;

pub enum ResourceCommand {
    CreateMesh {},
}

/// Resource manager is responsible for mapping and tracking GPU resources such as textures and buffers
#[derive(Debug, Default)]
pub struct ResourceManager {
    pub buffer_store: dare_containers::slot_map::SlotMap<Option<dagal::resource::Buffer<GPUAllocatorImpl>>>,
    pub image_store: dare_containers::slot_map::SlotMap<Option<dagal::resource::Image<GPUAllocatorImpl>>>
}

/// ErasedStore is a hashmap that stores boxed trait objects
struct ErasedStore {
    store: HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

/// Map asset handles to resource handles
#[derive(Debug, Default, Resource)]
pub struct AssetManagerToResourceManager {
    pub resource_map: HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
    pub geometries: HashMap<GeometryDescriptionHandle, ResourceHandle>,
    pub geometry_descriptions: HashMap<GeometryDescriptionHandle, GeometryDescription>,
    pub geometry_runtimes: HashMap<GeometryDescriptionHandle, std::sync::Arc<dare_assets::GeometryRuntime>>,
}
