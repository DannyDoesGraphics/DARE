use std::ops::{Deref, DerefMut};
use std::slice::{Iter, IterMut};
use crate::error::ContainerErrors;
use crate::prelude::Slot;

/// Regular slot map implementation

#[derive(Debug, PartialEq, Eq)]
pub struct InsertionSortSlotMap<T: Eq + PartialEq + PartialOrd + Ord> {
    // usize is a reference to the proxy slot index
    pub(crate) handle: super::SlotMap<T>,
}

impl<T: Eq + PartialEq + PartialOrd + Ord> Default for InsertionSortSlotMap<T> {
    fn default() -> Self {
        Self {
            handle: super::SlotMap::default()
        }
    }
}

impl<T: Eq + PartialEq + PartialOrd + Ord> Deref for InsertionSortSlotMap<T> {
    type Target = super::SlotMap<T>;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl<T: Eq + PartialEq + PartialOrd + Ord> DerefMut for InsertionSortSlotMap<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.handle
    }
}

impl<T: Eq + PartialEq + PartialOrd + Ord + std::fmt::Debug> InsertionSortSlotMap<T> {
    /// Insertion cost is O(n^2) and is very computationally expensive
    pub fn insertion_sort(&mut self, element: T) -> Result<Slot<T>, ContainerErrors> {
        let position_in_vec = self.handle.data.binary_search_by(|(probe, _)| {
            probe.cmp(&element)
        }).unwrap_or_else(|e| e);
        let mut free_slot = self.free_list.pop().unwrap_or_else(|| {
            let slot = Slot::new(self.data.len(), 0);
            self.slots.push(slot.clone());
            slot
        });
        free_slot.id = position_in_vec;
        let slot_len = self.slots.len() - 1;
        self.data.insert(position_in_vec, (element, slot_len));
        // update all mappings after
        {
            let indirect_indices: Vec<usize> = self.data[(position_in_vec + 1)..].iter().map(|(_, index)| *index).collect::<Vec<usize>>();
            for index in indirect_indices {
                self.slots.get_mut(index).unwrap()
                    .id += 1;
            }
        }

        // produce and out slot from mapping to the proxy slot
        let out_slot = Slot::new(
            self.slots.len() - 1,
            free_slot.generation
        );
        Ok(out_slot)
    }
}