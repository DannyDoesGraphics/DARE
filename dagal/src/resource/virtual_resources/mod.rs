mod eviction_policies;
mod traits;

use crate::resource::traits::Resource;
use crate::resource::virtual_resources::traits::EvictionPolicy;
/// # Virtual resources
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;

/// Represents a virtual_resources resource
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualResource {
    pub(crate) uid: u64,
    pub(crate) resource: TypeId,
}
impl<R: Resource + 'static> PartialEq<VirtualResourceTyped<R>> for VirtualResource {
    fn eq(&self, other: &VirtualResourceTyped<R>) -> bool {
        self.uid == other.uid && TypeId::of::<R>() == self.resource
    }
}
impl Hash for VirtualResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.resource.hash(state);
    }
}

/// Represents a virtual_resources resource handle that is strongly typed
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VirtualResourceTyped<R: Resource> {
    pub(crate) uid: u64,
    pub(crate) _phantom: PhantomData<R>,
}
impl<R: Resource + 'static> Hash for VirtualResourceTyped<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        TypeId::of::<R>().hash(state);
    }
}
impl<R: Resource + 'static> VirtualResourceTyped<R> {
    /// Attempts to turn a [`VirtualResource`] into a strongly typed [`VirtualResourceTyped`]
    pub fn from(res: VirtualResource) -> Option<Self> {
        if TypeId::of::<R>() == res.resource {
            Some(Self {
                uid: res.uid,
                _phantom: PhantomData::default(),
            })
        } else {
            None
        }
    }
}
impl<R: Resource + 'static> Into<VirtualResource> for VirtualResourceTyped<R> {
    fn into(self) -> VirtualResource {
        VirtualResource {
            uid: self.uid,
            resource: TypeId::of::<R>(),
        }
    }
}
impl<R: Resource> From<VirtualResource> for VirtualResourceTyped<R> {
    fn from(value: VirtualResource) -> Self {
        Self {
            uid: value.uid,
            _phantom: PhantomData::default(),
        }
    }
}
impl<R: Resource + 'static> PartialEq<VirtualResource> for VirtualResourceTyped<R> {
    fn eq(&self, other: &VirtualResource) -> bool {
        self.uid == other.uid && TypeId::of::<R>() == other.resource
    }
}

/// Storages all physical resources and creates a mapping to them from a virtual resource
#[derive(Debug, Default)]
struct VirtualResourceStorage<E: EvictionPolicy> {
    pub(crate) bindings: HashMap<VirtualResource, Box<dyn Any>>,
    pub(crate) eviction_policy: E,
}
impl<E: EvictionPolicy> VirtualResourceStorage<E> {
    pub fn new(policy: E) -> Self {
        Self {
            bindings: HashMap::new(),
            eviction_policy: policy,
        }
    }

    /// Insert a physical resource into the virtual resource manager
    pub fn insert<R: Resource + 'static>(
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
    pub fn get<R: Resource + 'static>(&mut self, virtual_resource: &VirtualResource) -> Option<&R> {
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
