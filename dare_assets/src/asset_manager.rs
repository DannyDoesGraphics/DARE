use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::{GeometryDescription, GeometryDescriptionHandle, MeshAsset, MeshHandle};

/// Asset manager is responsible for handling high-level asset operations.
#[derive(Debug, Resource, Default)]
pub struct AssetManager {
    ttl: u16,
    geometry_descriptions:
        dare_containers::slot_map::SlotMap<GeometryDescription, GeometryDescriptionHandle>,
    geometry_runtime:
        HashMap<GeometryDescriptionHandle, std::sync::Arc<crate::geometry::GeometryRuntime>>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
}

impl AssetManager {
    pub fn new(ttl: u16) -> Self {
        Self {
            ttl,
            ..Default::default()
        }
    }

    /// Create an entirely new geometry and ensures geometries are backed by a [`crate::geometry::GeometryRuntime`]
    pub fn create_geometry(
        &mut self,
        geometry: crate::GeometryDescription,
    ) -> crate::GeometryDescriptionHandle {
        let handle = self.geometry_descriptions.insert(geometry);
        let runtime = crate::geometry::GeometryRuntime {
            ttl: std::sync::atomic::AtomicU16::from(self.ttl),
            ..Default::default()
        };
        assert!(
            self.geometry_runtime
                .insert(handle, std::sync::Arc::new(runtime))
                .is_none(),
            "All runtimes should be None"
        );
        handle
    }

    /// Remove a geometry, return [`None`] if removing a non-existent geometry
    pub fn remove_geometry(
        &mut self,
        handle: crate::GeometryDescriptionHandle,
    ) -> Option<crate::GeometryDescription> {
        self.geometry_descriptions
            .remove(handle)
            .inspect(|_| {
                self.geometry_runtime.remove(&handle);
            })
            .ok()
    }
}
