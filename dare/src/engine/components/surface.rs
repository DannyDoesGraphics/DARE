use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use std::cmp::Ordering;

#[derive(Default, Clone, Debug)]
pub struct SurfaceBuilder {
    pub vertex_count: usize,
    pub index_count: usize,
    pub index_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub vertex_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub normal_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub tangent_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub uv_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
}

impl SurfaceBuilder {
    pub fn build(self) -> Surface {
        Surface {
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            index_buffer: self.index_buffer.unwrap(),
            vertex_buffer: self.vertex_buffer.unwrap(),
            normal_buffer: self.normal_buffer,
            tangent_buffer: self.tangent_buffer,
            uv_buffer: self.uv_buffer,
        }
    }
}

#[derive(becs::Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Surface {
    pub vertex_count: usize,
    pub index_count: usize,
    pub index_buffer: dare::asset::AssetHandle<dare::asset::assets::Buffer>,
    pub vertex_buffer: dare::asset::AssetHandle<dare::asset::assets::Buffer>,
    pub normal_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub tangent_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
    pub uv_buffer: Option<dare::asset::AssetHandle<dare::asset::assets::Buffer>>,
}

impl PartialOrd for Surface {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.index_count.cmp(&other.index_count).then_with(|| {
            self.vertex_count
                .cmp(&other.vertex_count)
                .then_with(|| Ordering::Equal)
        }))
    }
}
impl Ord for Surface {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Surface {
    /// Downgrades all handles
    pub fn downgrade(self) -> Self {
        Self {
            vertex_count: self.vertex_count,
            index_count: self.index_count,
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
