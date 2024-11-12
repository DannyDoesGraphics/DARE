use anyhow::Result;

use crate::slot::Slot;

pub struct SlotUnion<'a, T> {
    pub slot: Slot<T>,
    pub data: Option<&'a T>,
}

pub struct SlotUnionMut<'a, T> {
    pub slot: Slot<T>,
    pub data: Option<&'a mut T>,
}

pub trait Container<T: 'static> {
    type Slot;

    fn new() -> Self;

    fn insert(&mut self, element: T) -> Self::Slot;

    /// Check if the data is valid
    fn is_valid(&self, slot: &Self::Slot) -> bool;

    /// Remove the slot and get the underlying data
    fn remove(&mut self, slot: Self::Slot) -> Result<T>;

    /// Get total size of the data buffer
    fn total_data_len(&self) -> usize;

    /// Pass the slot and access an immutable reference of the underlying data
    fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Self::Slot, func: F) -> Result<R>;

    /// Pass the slot and access a mutable reference of the underlying data
    fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(&mut self, slot: &Self::Slot, func: F)
        -> Result<R>;

    fn iter<'b>(&'b self) -> impl Iterator<Item = SlotUnion<'b, T>>;

    fn iter_mut<'b>(&'b mut self) -> impl Iterator<Item = SlotUnionMut<'b, T>>;

    /// Filters with a predicate function
    fn retain<F: Fn(&T) -> bool>(&mut self, predicate: F);
}
