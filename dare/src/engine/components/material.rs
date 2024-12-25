use crate::prelude as dare;
use crate::render2::c::CMaterial;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, becs::Component)]
pub struct Material {
    pub albedo_factor: glam::Vec4,
}
impl Eq for Material {}
impl Hash for Material {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for i in self.albedo_factor.to_array() {
            i.to_bits().hash(state);
        }
    }
}
