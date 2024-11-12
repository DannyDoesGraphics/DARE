use crate::prelude as dare;
use bevy_ecs::prelude as becs;

#[derive(Default, Clone, Debug)]
pub struct SurfaceBuilder {
    pub vertex_count: usize,
    pub index_count: usize,
    pub first_index: usize,
    pub index_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub vertex_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub normal_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub tangent_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub uv_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
}

impl SurfaceBuilder {
    pub fn build(self) -> Surface {
        Surface {
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            first_index: self.first_index,
            index_buffer: self.index_buffer.unwrap(),
            vertex_buffer: self.vertex_buffer.unwrap(),
            normal_buffer: self.normal_buffer,
            tangent_buffer: self.tangent_buffer,
            uv_buffer: self.uv_buffer,
        }
    }
}

#[derive(becs::Component, Debug, Clone)]
pub struct Surface {
    pub vertex_count: usize,
    pub index_count: usize,
    pub first_index: usize,
    pub index_buffer: dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>,
    pub vertex_buffer: dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>,
    pub normal_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub tangent_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
    pub uv_buffer: Option<dare::asset2::AssetHandle<dare::asset2::implementations::Buffer>>,
}

impl Surface {
    /// Downgrades all handles
    pub fn downgrade(self) -> Self {
        Self {
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            first_index: self.first_index,
            index_buffer: self.index_buffer.downgrade(),
            vertex_buffer: self.vertex_buffer.downgrade(),
            normal_buffer: self.normal_buffer.map(|b| b.downgrade()),
            tangent_buffer: self.tangent_buffer.map(|b| b.downgrade()),
            uv_buffer: self.uv_buffer.map(|b| b.downgrade()),
        }
    }

    /// Upgrades all handles
    pub fn upgrade(self) -> Option<Self> {
        Some(Self {
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            first_index: self.first_index,
            index_buffer: self.index_buffer.upgrade()?,
            vertex_buffer: self.vertex_buffer.upgrade()?,
            normal_buffer: match self.normal_buffer {
                Some(b) => Some(b.upgrade()?),
                None => None,
            },
            tangent_buffer: match self.tangent_buffer {
                Some(b) => Some(b.upgrade()?),
                None => None,
            },
            uv_buffer: match self.uv_buffer {
                Some(b) => Some(b.upgrade()?),
                None => None,
            },
        })
    }
}
