use std::{collections::HashMap, hash::Hash};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MeshAsset {
    pub vertex_buffer: super::GeometryHandle,
    pub index_buffer: super::GeometryHandle,
    pub uv_buffers: HashMap<u32, super::GeometryHandle>,
}

impl Hash for MeshAsset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.vertex_buffer.hash(state);
        self.index_buffer.hash(state);
        let mut keys: Vec<u32> = self.uv_buffers.keys().cloned().collect();
        keys.sort();

        for key in keys {
            key.hash(state);
            self.uv_buffers[&key].hash(state);
        }
    }
}
