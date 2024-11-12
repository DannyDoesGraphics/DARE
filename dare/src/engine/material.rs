use crate::prelude as dare;
use crate::render2::c::CMaterial;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;

#[derive(Debug, Clone, becs::Component)]
pub struct Material {
    pub albedo_factor: glam::Vec4,
}
