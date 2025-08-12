use crate::allocators::Allocator;

/// A simple abstraction that combines a [`super::Image`] and [`super::ImageView`] into one struct

#[derive(Debug, PartialEq, Eq)]
pub struct Texture<A: Allocator> {
    pub image: super::Image<A>,
    pub image_view: super::ImageView,
}
