use crate::asset::prelude as asset;
use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;

/// Represents weak textures
#[derive(Debug, Clone)]
pub struct Texture<A: Allocator + 'static> {
    pub image: asset::ImageMetaData<A>,
    pub image_view: asset::ImageViewMetadata<A>,
}
