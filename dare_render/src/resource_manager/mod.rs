mod handle;

use dagal::allocators::GPUAllocatorImpl;
pub use handle::*;

use bevy_ecs::prelude::*;
use dare_assets::*;
use std::{collections::HashMap, hash::Hash};

pub enum ResourceCommand {
    CreateMesh {},
}

/// Resource manager is responsible for mapping and tracking GPU resources such as textures and buffers
#[derive(Debug, Default)]
pub struct ResourceManager {
    pub buffer_store:
        dare_containers::slot_map::SlotMap<Option<dagal::resource::Buffer<GPUAllocatorImpl>>>,
    pub image_store:
        dare_containers::slot_map::SlotMap<Option<dagal::resource::Image<GPUAllocatorImpl>>>,
}

/// Map asset handles to resource handles
#[derive(Debug, Resource)]
pub struct AssetManagerToResourceManager {
    /// Maps to physical gpu resources
    pub physical_resource_map:
        HashMap<GeometryDescriptionHandle, Box<dyn std::any::Any + Send + Sync>>,
    pub resource_manager: dare_assets::AssetManager,
    tombstone_ttl: u16,
}

impl AssetManagerToResourceManager {
    pub fn new(resource_manager: dare_assets::AssetManager, tombstone_ttl: u16) -> Self {
        Self {
            resource_manager,
            tombstone_ttl,
            physical_resource_map: HashMap::new(),
        }
    }

    pub fn tick(&mut self) {
        self.resource_manager
            .geometry_runtime
            .iter()
            .for_each(|(handle, runtime)| {
                runtime
                    .ttl
                    .fetch_update(
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Relaxed,
                        |ttl: u16| {
                            let old: u16 = ttl;
                            let mut ttl: u16 = ttl.saturating_sub(1);
                            runtime
                                .residency
                                .fetch_update(
                                    std::sync::atomic::Ordering::Relaxed,
                                    std::sync::atomic::Ordering::Acquire,
                                    |resident_state: u8| {
                                        if ttl == 0
                                            && resident_state
                                                == dare_assets::ResidentState::ResidentGPU as u8
                                        {
                                            ttl = self.tombstone_ttl;
                                            return Some(
                                                dare_assets::ResidentState::Unloading as u8
                                            );
                                        } else if ttl == 0
                                            && resident_state
                                                == dare_assets::ResidentState::Unloading as u8
                                        {
                                            self.physical_resource_map.remove(handle).unwrap();
                                            return Some(dare_assets::ResidentState::Unloaded as u8);
                                        } else {
                                            None
                                        }
                                    },
                                )
                                .ok();
                            if old != ttl { Some(ttl) } else { None }
                        },
                    )
                    .ok();
            })
    }

    /// If `create` is `true`, then a physical resource will be realized
    pub fn get_physical_resource<T>(&mut self, create: bool) {}
}
