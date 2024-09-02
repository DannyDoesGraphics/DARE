use anyhow::Result;

use crate::error::ContainerErrors;
use crate::prelude::{SlotUnion, SlotUnionMut};
use crate::slot::Slot;
use crate::traits::Container;

#[derive(Debug)]
pub struct FreeList<T: 'static> {
    data: Vec<Option<T>>,
    free_list: Vec<Slot<T>>,
}

impl<T: 'static> Container<T> for FreeList<T> {
    type Slot = Slot<T>;

    fn new() -> Self {
        Self {
            data: Vec::new(),
            free_list: Vec::new(),
        }
    }

    fn insert(&mut self, element: T) -> Slot<T> {
        let next_free_slot = self.free_list.pop().unwrap_or_else(|| {
            self.data.push(None);
            Slot::new(self.data.len(), 0)
        });
        *self.data.get_mut(next_free_slot.id()).unwrap() = Some(element);
        next_free_slot
    }

    fn is_valid(&self, slot: &Self::Slot) -> bool {
        self.data.get(slot.id()).is_some()
    }

    fn remove(&mut self, slot: Self::Slot) -> Result<T> {
        self.free_list.push(slot.clone());
        self.data
            .remove(slot.id())
            .ok_or(anyhow::Error::from(ContainerErrors::NonexistentSlot))
    }

    fn total_data_len(&self) -> usize {
        self.data.len()
    }

    fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Self::Slot, func: F) -> Result<R> {
        self.data
            .get(slot.id())
            .and_then(|data| data.as_ref())
            .map_or(
                Err(anyhow::Error::from(ContainerErrors::NonexistentSlot)),
                |data| Ok(func(data)),
            )
    }

    fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(
        &mut self,
        slot: &Self::Slot,
        func: F,
    ) -> anyhow::Result<R> {
        self.data
            .get_mut(slot.id())
            .and_then(|data| data.as_mut())
            .map_or(
                Err(anyhow::Error::from(ContainerErrors::NonexistentSlot)),
                |data| Ok(func(data)),
            )
    }

    fn iter(&self) -> impl Iterator<Item=SlotUnion<T>> {
        self.data.iter().enumerate().map(|(index, data)| SlotUnion {
            slot: Slot::new(index, 0),
            data: data.as_ref(),
        })
    }

    fn iter_mut(&mut self) -> impl Iterator<Item=SlotUnionMut<T>> {
        self.data
            .iter_mut()
            .enumerate()
            .map(|(index, data)| SlotUnionMut {
                slot: Slot::new(index, 0),
                data: data.as_mut(),
            })
    }

    fn filter_with<F: Fn(&T) -> bool>(&mut self, predicate: F) {
        for data_slot in self.data.iter_mut() {
            if let Some(data) = data_slot {
                if !predicate(data) {
                    *data_slot = None;
                }
            }
        }
    }
}

impl<T> Default for FreeList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FreeList<T> {
    pub fn with_capacity(free_list_capacity: usize, data_capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(data_capacity),
            free_list: Vec::with_capacity(free_list_capacity),
        }
    }
}
