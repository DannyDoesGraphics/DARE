use std::any::TypeId;
use std::hash::{Hash, Hasher};
/// Virtual resources are any form of rendering resource [`super::traits::Resource`] which contain
/// specialized resources.

use std::marker::PhantomData;
use derivative::Derivative;
use crate::resource::traits::Resource;

/// Untyped version of resource handlesdd
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct ResourceHandleUntyped {
    pub(crate) id: u32,
    pub(crate) generation: u32,
    pub(crate) type_id: TypeId,
}
impl ResourceHandleUntyped {
    pub(crate) fn new(id: u32, generation: u32, ty: TypeId) -> Self {
        Self {
            id,
            generation,
            type_id: ty
        }
    }

    /// Get resource id
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get resource generation
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// As typed
    pub fn as_typed<T: Resource + 'static>(&self) -> Option<ResourceHandle<T>> {
        if self.type_id == TypeId::of::<T>() {
            Some(ResourceHandle {
                id: self.id,
                generation: self.generation,
                _phantom: Default::default(),
            })
        } else {
            None
        }
    }
}
impl Hash for ResourceHandleUntyped {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}
impl<T: Resource + 'static> PartialEq<ResourceHandle<T>> for ResourceHandleUntyped {
    fn eq(&self, other: &ResourceHandle<T>) -> bool {
        self.id == other.id && self.generation == other.generation && self.type_id == TypeId::of::<T>()
    }
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, Clone)]
pub struct ResourceHandle<T: Resource + 'static> {
    pub(crate) id: u32,
    pub(crate) generation: u32,
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    _phantom: PhantomData<T>,
}
impl<T: Resource + 'static> ResourceHandle<T> {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self {
            id,
            generation,
            _phantom: Default::default(),
        }
    }

    /// Get resource id
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get resource generation
    pub fn generation(&self) -> u32 {
        self.generation
    }
}
impl<T: Resource + 'static> Hash for ResourceHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
        TypeId::of::<T>().hash(state);
    }
}
impl<T: Resource + 'static> Into<ResourceHandleUntyped> for ResourceHandle<T> {
    fn into(self) -> ResourceHandleUntyped {
        ResourceHandleUntyped {
            id: self.id,
            generation: self.generation,
            type_id: TypeId::of::<T>(),
        }
    }
}
impl<T: Resource + 'static> PartialEq<ResourceHandleUntyped> for ResourceHandle<T> {
    fn eq(&self, other: &ResourceHandleUntyped) -> bool {
        self.id == other.id && self.generation == other.generation && TypeId::of::<T>() == other.type_id
    }
}