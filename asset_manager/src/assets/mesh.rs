/// Logical representation of a mesh asset
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetMesh {
    pub name: String,
    pub vertex_count: usize,
    pub index_count: usize,
    pub vertex_buffer: super::AssetBuffer,
    pub index_buffer: super::AssetBuffer,
}

impl AssetMesh {
    /// Get # of vertices
    pub fn vertices(&self) -> usize {
        self.vertex_count
    }

    /// Get # of indices
    pub fn indices(&self) -> usize {
        self.index_count
    }
}
