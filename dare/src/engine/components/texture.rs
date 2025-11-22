use crate::prelude as dare;
use bevy_ecs::prelude::*;

/// A texture is an engine component which simply contains a reference to an image and a sampler
/// for said image
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Texture {
    pub asset_handle: dare::asset::AssetHandle<dare::asset::assets::Image>,
    pub sampler: dare::engine::components::Sampler,
}
