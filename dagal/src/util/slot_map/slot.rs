use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use derivative::Derivative;

#[derive(Copy, Ord, PartialOrd, Derivative, Default)]
#[derivative(Debug)]
pub struct Slot<T> {
    pub(super) id: u64,
    pub(super) generation: u64,
    #[derivative(Debug = "ignore")]
    pub(super) _marker: PhantomData<T>,
}

impl<T> Clone for Slot<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            generation: self.generation,
            _marker: Default::default(),
        }
    }
}

impl<T> PartialEq for Slot<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.generation == other.generation
    }
}

impl<T> Eq for Slot<T> {}

impl<T> Hash for Slot<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl<T> Slot<T> {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn new(id: u64, generation: Option<u64>) -> Self {
        Self {
            id,
            generation: generation.unwrap_or(0),
            _marker: Default::default(),
        }
    }
}

unsafe impl<T> Send for Slot<T> {}

unsafe impl<T> Sync for Slot<T> {}
