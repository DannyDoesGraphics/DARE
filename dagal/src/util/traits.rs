use std::fmt::Debug;
use std::hash::Hash;

use anyhow::Result;

use crate::util::Slot;

pub trait SlotTrait: Clone + PartialEq + Eq + Hash + Debug {
    /// Get id of slot
    fn id(&self) -> usize;

    /// Get generation of slot
    fn generation(&self) -> usize;
}

pub trait SlotMap<T> {
    type Slot;

    /// Get underlying slot map data
    fn get_data(&self) -> &[Self::Slot];

    /// Get the number of data entries regardless if they're empty or not
    fn all_slot_len(&self) -> usize;

    /// Determine if it is a valid slot map
    fn is_valid_slot(&self, slot: &Slot<T>) -> bool;

    /// Insert into slot map
    fn insert(&mut self, element: T) -> Slot<T>;

    /// Immutably access a slot's underlying data
    fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Slot<T>, func: F) -> Result<R>;

    /// Mutably access a slot's underlying data
    fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(&mut self, slot: &Slot<T>, func: F) -> Result<R>;
}