use crate::prelude as dare;
use bevy_ecs::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct Texture {
    pub asset_handle: dare::asset2::AssetHandle<dare::asset2::assets::Image>,
    pub sampler: dare::engine::components::Sampler,
}
