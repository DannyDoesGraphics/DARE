use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::{Geometry, GeometryHandle, MeshAsset, MeshHandle};

/// Asset manager is responsible for handling high-level asset operations.
#[derive(Debug, Resource, Default)]
pub struct AssetManager {
    ttl: u16,
    geometry_store: dare_containers::slot_map::SlotMap<Geometry, GeometryHandle>,
    geometry_runtime: HashMap<GeometryHandle, std::sync::Arc<crate::geometry::GeometryRuntime>>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
}

impl AssetManager {
    pub fn new(ttl: u16,) -> Self {
        Self {
            ttl,
            ..Default::default()
        }
    }
    
    /// Create an entirely new geometry and ensures geometries are backed by a [`crate::geometry::GeometryRuntime`]
    pub fn create_geometry(&mut self, geometry: crate::Geometry) -> crate::GeometryHandle {
        let handle = self.geometry_store.insert(geometry);
        let mut runtime = crate::geometry::GeometryRuntime::default();
        runtime.ttl = std::sync::atomic::AtomicU16::from(self.ttl);
        assert!(self.geometry_runtime.insert(handle, std::sync::Arc::new(runtime)).is_none(), "All runtimes should be None");
        handle
    }
    
    /// Remove a geometry, return [`None`] if removing a non-existent geometry
    pub fn remove_geometry(&mut self, handle: crate::GeometryHandle) -> Option<crate::Geometry> {
        self.geometry_store.remove(handle).and_then(|geometry| {
            self.geometry_runtime.remove(&handle);
            Ok(geometry)
        }).ok()
    }
}
