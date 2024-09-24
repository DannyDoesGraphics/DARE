use dashmap::iter::IterMut;
use dashmap::mapref::one::{Ref, RefMut};
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::hash::RandomState;

/// A dashmap which has type erasure
///
/// # References
/// We do not hand out any references in the erased storage dash map. Instead, to access the interior,
/// you must first use [`ErasedStorageDashMap::with`] to get a reference.
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

    pub fn with_or_default<T: 'static + Default, R, F: FnOnce(&T) -> R>(&self, f: F) -> R {
        match self.dash_map.get(&TypeId::of::<T>()) {
            None => {
                self.dash_map
                    .insert(TypeId::of::<T>(), Box::new(T::default()));
                // SAFETY: we know this must exist
                self.dash_map
                    .get(&TypeId::of::<T>())
                    .map(|data| data.downcast_ref::<T>().map(f))
                    .flatten()
                    .unwrap()
            }
            Some(data) => data.downcast_ref::<T>().map(f).unwrap(),
        }
    }

    pub fn with_mut<T: 'static, R, F: FnOnce(&mut T) -> R>(&self, f: F) -> Option<R> {
        self.dash_map
            .get_mut(&TypeId::of::<T>())
            .and_then(|mut data| data.downcast_mut::<T>().map(f))
    }

    pub fn iter<'a>(
        &'a self,
    ) -> IterMut<'a, TypeId, Box<dyn Any>, RandomState, DashMap<TypeId, Box<dyn Any>>> {
        self.dash_map.iter_mut()
    }

    pub fn get<'a, T: 'static>(&'a self) -> Option<Ref<'a, TypeId, Box<dyn Any>>> {
        self.dash_map.get(&TypeId::of::<T>())
    }

    pub fn get_mut<'a, T: 'static>(&'a self) -> Option<RefMut<'a, TypeId, Box<dyn Any>>> {
        self.dash_map.get_mut(&TypeId::of::<T>())
    }

    pub fn handle(&self) -> &DashMap<TypeId, Box<dyn Any>> {
        &self.dash_map
    }

    /// Remove from erased storage
    pub fn remove<T: 'static>(&mut self) -> Option<Box<T>> {
        self.dash_map
            .remove(&TypeId::of::<T>())
            .and_then(|(_type_id, data)| data.downcast::<T>().ok())
    }
}
