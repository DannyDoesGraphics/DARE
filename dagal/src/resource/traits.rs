use crate::traits::Destructible;
use anyhow::Result;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource<'a>: Destructible + Sized {
    /// Necessary create info
    type CreateInfo: 'a;
    /// Type of underlyin where Self::CreateInfo<'a>: 'a;g VkObject the struct is representing
    type HandleType;

    /// Attempt to create a new resource given the [`Self::CreateInfo`] struct
    fn new(create_info: Self::CreateInfo) -> Result<Self>
    where
        Self: Sized;

    /// Get a reference to the underlying VkObject.
    fn get_handle(&self) -> &Self::HandleType;

    /// Get a copy to the underlying VkObject
    fn handle(&self) -> Self::HandleType;

    /// Get the name of the resource
    fn get_name() -> String;
}
