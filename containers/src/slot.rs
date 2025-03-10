use derivative::Derivative;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, Hash)]
pub struct Slot<T> {
    pub(crate) id: u64,
    pub(crate) generation: u64,
    #[derivative(Debug = "ignore", PartialEq = "ignore", Hash = "ignore")]
    _marker: PhantomData<T>,
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

impl<T> Slot<T> {
    pub fn new(id: u64, generation: u64) -> Self {
        Self {
            id,
            generation,
            _marker: Default::default(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// # Safety
    /// This allows you to arbitrarily change the generic
    pub unsafe fn transmute<A>(self) -> Slot<A> {
        Slot::new(self.id, self.generation)
    }

    /// # Safety
    /// No type safety guarantees are made here
    pub unsafe fn transmute_ref<A>(&self) -> &Slot<A> {
        unsafe { std::mem::transmute(self) }
    }
}
