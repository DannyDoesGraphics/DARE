use crate::buffer::Buffer;
use std::collections::HashMap;

/// Logical description of a mesh made up of multiple geometry slices.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Mesh {
    pub vertex_buffer: crate::AssetHandle<Buffer>,
    pub normal_buffer: crate::AssetHandle<Buffer>,
    pub index_buffer: crate::AssetHandle<Buffer>,
    pub uv_buffers: HashMap<u32, crate::AssetHandle<Buffer>>,
}

impl std::hash::Hash for Mesh {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.vertex_buffer.hash(state);
        self.normal_buffer.hash(state);
        self.index_buffer.hash(state);
        // TODO: this is not optimal at all
        for (index, uv0) in self.uv_buffers.iter() {
            index.hash(state);
            uv0.hash(state);
        }
    }
}
impl crate::Asset for Mesh {}
