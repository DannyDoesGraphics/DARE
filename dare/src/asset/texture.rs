use dagal::allocators::Allocator;
use dagal::resource;
use dare_containers::prelude as containers;
use crate::asset::prelude as asset;

/// Represents weak textures
#[derive(Debug, Clone)]
pub struct WeakTexture<A: Allocator + 'static> {
    pub image: asset::WeakAssetRef<asset::Image<A>>,
    pub image_view: asset::WeakAssetRef<asset::ImageView<A>>,
}

impl<A: Allocator + 'static> WeakTexture<A> {
    pub fn upgrade(&self) -> Option<StrongTexture<A>> {
        Some(StrongTexture {
            image: self.image.upgrade()?,
            image_view: self.image_view.upgrade()?,
        })
    }
}

/// Represents a texture with strong references to images and image views
#[derive(Debug, Clone)]
pub struct StrongTexture<A: Allocator + 'static> {
    pub image: asset::StrongAssetRef<asset::Image<A>>,
    pub image_view: asset::StrongAssetRef<asset::ImageView<A>>,
}
