use std::{any::TypeId, fmt::Debug, hash::Hash, marker::PhantomData};

use crate::resource::traits::Resource;

mod collection;

pub use collection::VirtualResourceCollection;

/// Similar to [`UntypedVirtualResource`], however, does not maintain a generation counter.
/// Useful for detecting if we're reusing the same resource handle twice erroneously
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub(crate) struct UntypedNoGenerationVirtualResource {
    id: u32,
    marker: TypeId,
}
impl From<UntypedVirtualResource> for UntypedNoGenerationVirtualResource {
    fn from(value: UntypedVirtualResource) -> Self {
        Self {
            id: value.id,
            marker: value.marker,
        }
    }
}
impl<A: Resource + 'static> From<VirtualResource<A>> for UntypedNoGenerationVirtualResource {
    fn from(value: VirtualResource<A>) -> Self {
        Self {
            id: value.id,
            marker: TypeId::of::<A>(),
        }
    }
}

/// [`VirtualResource`] but does not contain a viral generic type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct UntypedVirtualResource {
    id: u32,
    generation: u32,
    marker: TypeId,
}
impl<A: Resource + 'static> From<VirtualResource<A>> for UntypedVirtualResource {
    fn from(value: VirtualResource<A>) -> Self {
        Self {
            id: value.id,
            generation: value.generation,
            marker: TypeId::of::<A>(),
        }
    }
}
impl UntypedVirtualResource {
    pub(crate) fn into_typed<A: Resource + 'static>(self) -> Option<VirtualResource<A>> {
        if self.marker == TypeId::of::<A>() {
            Some(VirtualResource {
                id: self.id,
                generation: self.generation,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

pub struct VirtualResource<A: Resource + 'static> {
    id: u32,
    generation: u32,
    _marker: PhantomData<A>,
}

impl<A: Resource + 'static> Debug for VirtualResource<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualResource")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .finish()
    }
}
impl<A: Resource + 'static> Clone for VirtualResource<A> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A: Resource + 'static> Copy for VirtualResource<A> {}
impl<A: Resource + 'static> PartialEq for VirtualResource<A> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.generation == other.generation
    }
}
impl<A: Resource + 'static> Eq for VirtualResource<A> {}
impl<A: Resource + 'static> Hash for VirtualResource<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl<A: Resource + 'static> VirtualResource<A> {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self {
            id,
            generation,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}
