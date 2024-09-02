use dashmap::iter::IterMut;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::hash::RandomState;

/// A dashmap which has type erasure
#[derive(Debug, Default)]
pub struct ErasedStorageDashMap {
    dash_map: DashMap<TypeId, Box<dyn Any>>,
}

unsafe impl Send for ErasedStorageDashMap {}
unsafe impl Sync for ErasedStorageDashMap {}

impl ErasedStorageDashMap {
    pub fn new() -> Self {
        Self {
            dash_map: DashMap::new(),
        }
    }

    /// Check if there exists a key for the type
    pub fn contains_key<T: 'static>(&self) -> bool {
        self.dash_map.contains_key(&TypeId::of::<T>())
    }

    /// Insert an item into
    pub fn insert<T: 'static>(&self, element: T) {
        self.dash_map.insert(TypeId::of::<T>(), Box::new(element));
    }

    pub fn with<T: 'static, R, F: FnOnce(&T) -> R>(&self, f: F) -> Option<R> {
        self.dash_map
            .get(&TypeId::of::<T>())
            .and_then(|data| data.downcast_ref::<T>().map(f))
    }

    pub fn with_mut<T: 'static, R, F: FnOnce(&mut T) -> R>(&self, f: F) -> Option<R> {
        self.dash_map
            .get_mut(&TypeId::of::<T>())
            .and_then(|mut data| data.downcast_mut::<T>().map(f))
    }

    pub fn iter<'a>(&'a self) -> IterMut<'a, TypeId, Box<dyn Any>, RandomState, DashMap<TypeId, Box<dyn Any>>> {
        self.dash_map
            .iter_mut()
    }

    /// Remove from erased storage
    pub fn remove<T: 'static>(&mut self) -> Option<Box<T>> {
        self.dash_map
            .remove(&TypeId::of::<T>())
            .and_then(|(_type_id, data)| data.downcast::<T>().ok())
    }
}
