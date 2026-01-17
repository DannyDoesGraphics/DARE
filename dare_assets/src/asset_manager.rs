use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::{GeometryDescription, GeometryDescriptionHandle, MeshAsset, MeshHandle};

/// Commands to send to the render side of the geometry manager
#[derive(Message)]
pub enum RenderAssetCommand {
    CreateGeometry {
        handle: GeometryDescriptionHandle,
        description: GeometryDescription,
        runtime: crate::GeometryRuntime,
    },
    DestroyGeometry {
        handle: GeometryDescriptionHandle,
    },
    CreateMesh {
        handle: MeshHandle,
        mesh: MeshAsset,
    },
    DestroyMesh {
        handle: MeshHandle,
    },
}

/// Asset manager is responsible for handling high-level asset operations.
///
/// TO-DO: fix hashmap usage as it defeats the purpose of slot map O(1) look up times
#[derive(Debug, Resource, Default)]
pub struct AssetManager {
    ttl: u16,
    geometry_descriptions:
        dare_containers::slot_map::SlotMap<GeometryDescription, GeometryDescriptionHandle>,
    geometry_runtime:
        HashMap<GeometryDescriptionHandle, std::sync::Arc<crate::geometry::GeometryRuntime>>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
}
unsafe impl Send for AssetManager {}

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
