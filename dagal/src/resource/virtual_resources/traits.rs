use crate::resource::virtual_resources::VirtualResource;
use std::any::Any;
use std::collections::HashMap;

/// Defines a policy used to decide what happens to physical elements in a virtual resource
pub trait EvictionPolicy {
    type ResourceHandle;

    /// Invoked on every insertion performed
    fn on_insert(&mut self, key: VirtualResource) -> Self::ResourceHandle;

    /// Invoked on every access performed
    fn on_access(&mut self, key: &VirtualResource);

    /// Invoked to remove keys from the given storage
    fn evict(&mut self, storage: &mut HashMap<VirtualResource, Box<dyn Any>>);
}
