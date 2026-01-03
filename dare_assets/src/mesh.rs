use std::collections::HashMap;

use crate::GeometryDescriptionHandle;

/// Logical description of a mesh made up of multiple geometry slices.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MeshAsset {
    pub vertex_buffer: GeometryDescriptionHandle,
    pub normal_buffer: GeometryDescriptionHandle,
    pub index_buffer: GeometryDescriptionHandle,
    pub uv_buffers: HashMap<u32, GeometryDescriptionHandle>,
}

impl std::hash::Hash for MeshAsset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.vertex_buffer.hash(state);
        self.normal_buffer.hash(state);
        self.index_buffer.hash(state);
        if let Some(uv0) = self.uv_buffers.get(&0) {
            uv0.hash(state);
        }
    }
}
