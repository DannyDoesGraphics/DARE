
use crate::resource::traits::Resource;
/// # Virtual resources
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Represents a virtual_resources resource
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

