use derivative::Derivative;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait Slot: Debug + Clone + Send + PartialEq + Eq + Hash {
    /// Identity of the slot
    fn id(&self) -> u64;
    /// Set id
    fn set_id(&mut self, id: u64);
    /// New with slot
    fn new(id: u64) -> Self;
}

pub trait SlotWithGeneration: Slot {
    /// Generation of the slot
    fn generation(&self) -> u64;
    /// Set generation
    fn set_generation(&mut self, generation: u64);
    /// New slot with generation
    fn new_with_gen(id: u64, generation: u64) -> Self;
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq)]
pub struct DefaultSlot<T> {
    pub(crate) id: u64,
    pub(crate) generation: u64,
    _marker: PhantomData<T>,
}
unsafe impl<T> Send for DefaultSlot<T> {}
impl<T> Clone for DefaultSlot<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            generation: self.generation,
            _marker: Default::default(),
        }
    }
}
impl<T> std::hash::Hash for DefaultSlot<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let raw: u128 = (self.id as u128) << 64 | self.generation as u128;
        state.write_u128(raw);
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

impl<T> Slot for DefaultSlot<T> {
    fn id(&self) -> u64 {
        self.id
    }

    fn set_id(&mut self, id: u64) {
        self.id = id;
    }

    fn new(id: u64) -> Self {
        Self {
            id,
            generation: 0,
            _marker: Default::default(),
        }
    }
}

impl<T> SlotWithGeneration for DefaultSlot<T> {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    fn new_with_gen(id: u64, generation: u64) -> Self {
        Self {
            id,
            generation,
            _marker: Default::default(),
        }
    }
}
