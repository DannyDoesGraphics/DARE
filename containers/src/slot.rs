use derivative::Derivative;
use std::marker::PhantomData;

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, Hash)]
pub struct DefaultSlot<T> {
    pub(crate) id: u64,
    pub(crate) generation: u64,
    #[derivative(Debug = "ignore", PartialEq = "ignore", Hash = "ignore")]
    _marker: PhantomData<T>,
}

impl<T> Clone for DefaultSlot<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            generation: self.generation,
            _marker: Default::default(),
        }
    }
}

impl<T> DefaultSlot<T> {
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
    pub unsafe fn transmute<A>(self) -> DefaultSlot<A> {
        DefaultSlot::new(self.id, self.generation)
    }

    /// # Safety
    /// No type safety guarantees are made here
    pub unsafe fn transmute_ref<A>(&self) -> &DefaultSlot<A> {
        unsafe { std::mem::transmute(self) }
    }
}
