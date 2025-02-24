
use crate::resource::traits::Resource;
/// # Virtual resources
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;

/// Represents a virtual_resources resource
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualResource {
    pub uid: u64,
    pub gen: u64,
    pub type_id: TypeId,
}
impl AsRef<VirtualResource> for VirtualResource {
    fn as_ref(&self) -> &VirtualResource {
        self
    }
}
impl<R: Resource + 'static> PartialEq<VirtualResourceTyped<R>> for VirtualResource {
    fn eq(&self, other: &VirtualResourceTyped<R>) -> bool {
        self.uid == other.uid && TypeId::of::<R>() == self.type_id
    }
}
impl Hash for VirtualResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.type_id.hash(state);
    }
}

/// Represents a virtual_resources resource handle that is strongly typed
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VirtualResourceTyped<R: Resource> {
    pub uid: u64,
    pub gen: u64,
    pub _phantom: PhantomData<R>,
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
        if TypeId::of::<R>() == res.type_id {
            Some(Self {
                uid: res.uid,
                gen: res.gen,
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
            gen: self.gen,
            type_id: TypeId::of::<R>(),
        }
    }
}
impl<R: Resource> From<VirtualResource> for VirtualResourceTyped<R> {
    fn from(value: VirtualResource) -> Self {
        Self {
            uid: value.uid,
            gen: value.gen,
            _phantom: PhantomData::default(),
        }
    }
}
impl<R: Resource + 'static> PartialEq<VirtualResource> for VirtualResourceTyped<R> {
    fn eq(&self, other: &VirtualResource) -> bool {
        self.uid == other.uid && TypeId::of::<R>() == other.type_id
    }
}

/// Indicates an instance of a virtual resource that when dropped, will immediately remove the underlying
/// resource key it holds
#[derive(Debug)]
pub struct VirtualResourceDrop {
    pub(crate) resource: VirtualResource,
    pub(crate) send_drop: crossbeam_channel::Sender<VirtualResource>,
}
impl AsRef<VirtualResource> for VirtualResourceDrop {
    fn as_ref(&self) -> &VirtualResource {
        &self.resource
    }
}
impl Deref for VirtualResourceDrop {
    type Target = VirtualResource;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}
impl PartialEq for VirtualResourceDrop {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}
impl PartialEq<VirtualResource> for VirtualResourceDrop {
    fn eq(&self, other: &VirtualResource) -> bool {
        *self == *other
    }
}
impl Drop for VirtualResourceDrop {
    fn drop(&mut self) {
        self.send_drop.send(self.resource).unwrap();
    }
}