use crate::traits::Destructible;
use anyhow::Result;
use std::fmt::Debug;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource<'a>: Destructible + Debug + Sized {
    type CreateInfo;

    /// Attempt to create a new resource given the [`Self::CreateInfo`] struct
    fn new(create_info: Self::CreateInfo) -> Result<Self>;

    /// Get a reference to the underlying VkObject.
    fn get_handle(&self) -> &Self::CreateInfo;

    /// Get a copy to the underlying VkObject
    fn handle(&self) -> Self::CreateInfo;

    /// Get the name of the resource
    fn get_name() -> &'a str;
}
