use std::fmt::Debug;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use anyhow::Result;

/// Represents a slot map structure
///
///
/// A slot map effectively hands out keys which map to internal keys which themselves map to the
/// underlying data the map slot represents. In other words, a map slot maps keys to data.
///
///
/// It's performance characteristic are such that get/erase/insert are O(1) operations. **Unless**
/// inserting must allocate room for a new slot which then it becomes O(n).
pub struct SlotMap<T: Send + Sync> {
    /// Internal slots that act as a "pointer" between data and external keys
    indices: Vec<Slot>,

    /// Holds the underlying data
    data: Vec<RwLock<T>>,

    /// References indices that map to the data
    erase: Vec<usize>,

    /// A queue containing free indices
    free_queue: Vec<usize>,
}

impl<T: Send + Sync> SlotMap<T> {
    /// Create a new slot map. You should generally prefer to use [`SlotMap::new_with_capacity`]
    /// as the largest advantage of slot maps is not having to constantly allocate new memory and
    /// re-using old memory.
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
            data: Vec::new(),
            erase: Vec::new(),
            free_queue: Vec::new(),
        }
    }

    /// Create a new slot map with a certain capacity
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            indices: Vec::with_capacity(capacity),
            data: Vec::with_capacity(capacity),
            erase: Vec::with_capacity(capacity),
            free_queue: Vec::with_capacity(capacity),
        }
    }

    /// Returns [`Ok`] if the slot is valid
    pub fn validate_slot(&self, slot: &Slot) -> Result<()> {
        if let Some(index) = self.indices.get(slot.index) {
            if index.generation != slot.generation {
                return Err(anyhow::Error::from(errors::Errors::InvalidSlot));
            }
        } else {
            return Err(anyhow::Error::from(errors::Errors::InvalidSlot));
        }
        Ok(())
    }

    /// Insert new data into the slot map
    pub fn insert(&mut self, data: T) -> Slot {
        if self.free_queue.len() == 0 {
            // no room, allocate more
            self.indices.push(Slot {
                index: self.data.len(),
                generation: 0,
            });
            self.free_queue.push(self.indices.len() - 1);
        }
        let next_free_indices = self.free_queue.remove(0);
        // generate a key to return back
        let mut key: Slot = self.indices.get(next_free_indices).unwrap().clone();
        key.index = next_free_indices;

        // create the data and update the indices index to point to it
        self.data.push(RwLock::new(data));
        self.indices.get_mut(next_free_indices).unwrap().index = self.data.len() - 1;
        self.erase.push(next_free_indices);

        key
    }

    /// Attempts to do a mutable lock the data prior to invoking [`erase`](Self::erase). If it
    /// fails, it returns an Err.
    pub fn try_lock_erase(&mut self, slot: Slot) -> Result<T> {
        self.validate_slot(&slot)?;

        let _unused = self
            .data
            .get(self.indices.get(slot.index).unwrap().index)
            .unwrap()
            .try_write()
            .map_err(|_| anyhow::Error::from(dagal::DagalError::PoisonError))?;
        drop(_unused);
        let handle = self.erase(slot)?;
        Ok(handle.into_inner().unwrap())
    }

    /// Erase data from the slot map effectively removing it entirely.
    ///
    /// **This only ensures that no new references are made to the data, but does not make
    /// checks regarding existing ones.**
    ///
    /// See [`try_lock_erase`](Self::try_lock_erase) for an erase one that does a check prior to
    /// erasure.
    pub fn erase(&mut self, slot: Slot) -> Result<RwLock<T>> {
        // validate generation
        self.validate_slot(&slot)?;
        // update generation
        self.indices.get_mut(slot.index).unwrap().generation += 1;

        // swap the to be removed to the last element and drop the last element
        let data_index = self.indices.get(slot.index).unwrap().index;
        let last_data_index = self.data.len() - 1;

        // swap
        assert_eq!(self.data.len(), self.erase.len());
        if last_data_index != data_index {
            self.data.swap(data_index, last_data_index);
            self.erase.swap(data_index, last_data_index);
        }
        let removed_data = self.data.pop().unwrap();
        self.erase.pop();
        if data_index != last_data_index {
            // update the swapped elements
            let swapped_slot_index = *self.erase.get(data_index).unwrap();
            let swapped_slot = self.indices.get_mut(swapped_slot_index).unwrap();
            swapped_slot.index = data_index;
        }
        // update the index
        self.free_queue.push(slot.index);
        Ok(removed_data)
    }

    /// Retrieve the underlying read write lock to the data the slot is mapped to
    pub fn get_rw(&self, slot: &Slot) -> Result<&RwLock<T>> {
        self.validate_slot(slot)?;

        let indices_slot = self.indices.get(slot.index).unwrap();
        Ok(self.data.get(indices_slot.index).unwrap())
    }

    /// Retrieve the data that maps to the slot directly
    pub fn get(&self, slot: &Slot) -> Result<RwLockReadGuard<T>> {
        self.get_rw(slot)?
            .read()
            .map_err(|_| return anyhow::Error::from(errors::Errors::Poisoned))
    }

    /// Retrieve the data that maps to the slot directly as a mutable rw access
    pub fn get_mut(&self, slot: &Slot) -> Result<RwLockWriteGuard<T>> {
        self.get_rw(slot)?
            .write()
            .map_err(|_| return anyhow::Error::from(errors::Errors::Poisoned))
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Slot {
    index: usize,
    generation: usize,
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Errors {
        #[error("Invalid slot key given to slot map")]
        InvalidSlot,

        #[error("Poisoned")]
        Poisoned,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn create_retrieve_single() {
        // Test creating and retrieving a single item from a slot map
        let mut slot_map = crate::slot_map::SlotMap::new();
        let handle = slot_map.insert(1);
        assert_eq!(1, *slot_map.get(&handle).unwrap());
    }

    #[test]
    fn create_retrieve_multiple() {
        // Test creating and retrieving a multiple items from a slot map
        let mut slot_map: crate::slot_map::SlotMap<u32> = crate::slot_map::SlotMap::new();
        let mut handles: Vec<crate::slot_map::Slot> = Vec::with_capacity(10);
        for i in 0u32..10u32 {
            handles.push(slot_map.insert(i));
        }
        // retrieve arbitrary numbers
        for (i, handle) in handles.iter().enumerate() {
            assert_eq!(*slot_map.get(handle).unwrap(), i as u32);
        }
    }

    #[test]
    fn create_retrieve_and_remove_multiple() {
        // Test creating and retrieving a multiple items from a slot map
        let mut slot_map: crate::slot_map::SlotMap<u32> = crate::slot_map::SlotMap::new();
        let mut handles: Vec<crate::slot_map::Slot> = Vec::with_capacity(10);
        for i in 0u32..10u32 {
            handles.push(slot_map.insert(i));
        }
        // Remove numbers which are even
        for i in 0u32..10u32 {
            if i % 2 == 0 {
                slot_map
                    .erase(handles.get(i as usize).unwrap().clone())
                    .unwrap();
            }
        }
        // retrieve arbitrary numbers
        for (i, handle) in handles.iter().enumerate() {
            if i % 2 == 1 {
                assert_eq!(*slot_map.get(handle).unwrap(), i as u32);
            } else {
                assert!(slot_map.validate_slot(handle).is_err());
            }
        }
        // Validate remaining items in slot map
        for handle in handles.iter() {
            if let Ok(handle) = slot_map.get(handle) {
                let v = *handle;
                assert_eq!(v % 2, 1)
            }
        }
    }
}
