use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::{DefaultHasher, Hash, Hasher},
    marker::PhantomData,
};

use crate::resource::traits::Resource;

/// Opaque pointer to an underlying resource managed by the render graph
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct VirtualResource<A: Resource> {
    id: u64,
    ty: TypeId,
}
impl<A: Resource> Hash for VirtualResource<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.ty.hash(state);
    }
}

impl<A: Resource> VirtualResource<A> {
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            ty: TypeId::of::<A>(),
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// Contains the virtual resource mappings to their original underlying representation
pub struct VirtualResourceContainer {
    ids: std::collections::HashMap<TypeId, u64>,
    map: std::collections::HashMap<u64, Box<dyn Any>>,
}

impl VirtualResourceContainer {
    fn compute_hash<A: Resource>(handle: &VirtualResource<A>) -> u64 {
        let mut hasher = DefaultHasher::default();
        handle.hash(&mut hasher);
        hasher.finish()
    }
    pub fn get<A: Resource>(&self, handle: &VirtualResource<A>) -> Option<&A> {
        self.map
            .get(&Self::compute_hash(handle))
            .and_then(|boxed| boxed.downcast_ref::<A>())
    }

    pub fn get_mut(&mut self, handle: &VirtualResource<A>) -> Option<&mut A> {
        self.map
            .get_mut(&Self::compute_hash(handle))
            .and_then(|boxed| boxed.downcast_mut::<A>())
    }

    pub(crate) fn insert<A: Resource>(&mut self, resource: A) -> VirtualResource<A> {
        let mut id = self.ids.entry(TypeId::of::<A>()).or_default();
        let virtual_resource = VirtualResource::new::<A>(*id);
        id += 1;
        self.map
            .insert(Self::compute_hash(&virtual_resource), Box::new(resource));
        virtual_resource
    }

    pub(crate) fn remove<A: Resource>(&mut self, handle: VirtualResource<A>) -> Option<A> {
        self.map
            .remove(&Self::compute_hash(&handle))
            .and_then(|boxed| boxed.downcast::<A>().ok())
    }
}
