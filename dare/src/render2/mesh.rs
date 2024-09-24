use crate::asset::prelude as asset;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use std::sync::Arc;

/// Deals with a render mesh
#[derive(Debug, Clone, becs::Component)]
pub struct Mesh<A: Allocator + 'static> {
    indices: asset::StrongAssetRef<asset::Buffer<A>>,
    vertices: asset::StrongAssetRef<asset::Buffer<A>>,
    normals: Option<asset::StrongAssetRef<asset::Buffer<A>>>,
    uvs: Arc<[asset::StrongAssetRef<asset::Buffer<A>>]>,
}
