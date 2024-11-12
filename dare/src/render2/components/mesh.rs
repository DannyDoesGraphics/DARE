use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use std::sync::Arc;

#[derive(PartialEq, Eq, Hash)]
pub struct Mesh {
    vertex_buffer: dare::asset2::AssetId<dare::asset2::assets::Buffer>,
    index_buffer: dare::asset2::AssetId<dare::asset2::assets::Buffer>,
}
