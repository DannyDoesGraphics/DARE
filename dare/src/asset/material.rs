use super::prelude as asset;
use dagal::allocators::Allocator;

#[derive(Debug, Clone)]
pub struct Material<A: Allocator + 'static> {
    pub albedo: asset::WeakTexture<A>,
    pub normal: asset::WeakTexture<A>,
    pub metallic_roughness: asset::WeakTexture<A>,
}
