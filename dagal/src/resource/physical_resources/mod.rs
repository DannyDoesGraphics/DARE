pub mod eviction_policies;
pub mod traits;
pub use traits::*;
pub use eviction_policies::*;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::any::{Any, TypeId};
use std::marker::PhantomData;
use crate::resource::traits::Resource;
use crate::resource::virtual_resources::{VirtualResource, VirtualResourceTyped};

/// Stores all physical resources and creates a mapping to them from a virtual resource
#[derive(Debug, Default)]
pub struct PhysicalResourceBindings<R: Resource + 'static, E: EvictionPolicy> {
    pub(crate) bindings: HashMap<VirtualResource, Box<dyn Any>>,
    pub(crate) eviction_policy: E,
    _phantom: PhantomData<R>,
}

impl<R: Resource + 'static, E: EvictionPolicy> PhysicalResourceBindings<R, E> {
    pub fn new(policy: E) -> Self {
        Self {
            bindings: HashMap::new(),
            eviction_policy: policy,
            _phantom: Default::default(),
        }
    }

    /// Insert a physical resource into the virtual resource manager
    pub fn insert(
        &mut self,
        resource: R,
    ) -> Option<VirtualResourceTyped<R>> {
        let virtual_resource = VirtualResource {
            uid: {
                let mut hasher = DefaultHasher::new();
                resource.hash(&mut hasher);
                hasher.finish()
            },
            resource: TypeId::of::<R>(),
        };
        // key exists, no more
        if self.bindings.contains_key(&virtual_resource) {
            return None;
        }
        self.bindings
            .insert(virtual_resource.clone(), Box::new(resource));
        Some(virtual_resource.into())
    }

    /// Get a virtual_resource resource (if it exists)
    pub fn get(&mut self, virtual_resource: &VirtualResource) -> Option<&R> {
        self.eviction_policy.on_access(virtual_resource);
        match self.bindings.get(virtual_resource) {
            None => None,
            Some(b) => b.downcast_ref(),
        }
    }

    /// Perform a flush on the virtual storage to ensure we can rid physical resources deemed
    /// useless by eviction policy
    pub fn flush(&mut self) {
        self.eviction_policy.evict(&mut self.bindings);
    }
}