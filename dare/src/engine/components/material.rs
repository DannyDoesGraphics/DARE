use crate::prelude as dare;
use crate::render::c::CMaterial;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use gltf::material::AlphaMode;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, becs::Component)]
pub struct Material {
    pub albedo_factor: glam::Vec4,
    pub albedo_texture: Option<dare::engine::components::Texture>,
    pub alpha_mode: gltf::material::AlphaMode,
}
impl Eq for Material {}
impl Hash for Material {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for i in self.albedo_factor.to_array() {
            i.to_bits().hash(state);
        }
        self.albedo_texture.hash(state);
    }
}
impl PartialOrd for Material {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.alpha_mode, other.alpha_mode) {
            (AlphaMode::Opaque, AlphaMode::Opaque) => Some(Ordering::Equal),
            (AlphaMode::Opaque, _) => Some(Ordering::Greater),
            (_, AlphaMode::Opaque) => Some(Ordering::Less),
            (_, _) => Some(Ordering::Equal),
        }
    }
}
impl Ord for Material {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
