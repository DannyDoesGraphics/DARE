/// Describes Vulkan resources which can be destroyed
pub trait Destructible {
    /// Destroy the resource
    fn destroy(&mut self);
}

pub trait AsRaw {
    type RawType: Copy + Clone;

    /// Get a reference to the raw value
    unsafe fn as_raw(&self) -> &Self::RawType;

    /// Get mutable reference to the raw value
    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType;

    /// Get underlying raw value
    unsafe fn raw(self) -> Self::RawType;
}
