use super::prelude as asset;
use dagal::allocators::Allocator;

#[derive(Debug, Clone)]
pub struct Material<A: Allocator> {
    pub albedo: asset::Texture<A>,
    pub normal: asset::Texture<A>,
    pub metallic_roughness: asset::Texture<A>,
}
