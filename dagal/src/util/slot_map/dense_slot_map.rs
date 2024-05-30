use std::collections::VecDeque;
use std::fmt::Debug;
use std::slice::IterMut;

use anyhow::Result;

use crate::util::slot_map::Slot;

/// Represents a slot map structure
///
///
/// A slot map effectively hands out keys which map to internal keys which themselves map to the
/// underlying data the map slot represents. In other words, a map slot maps handles to data.
///
/// For more information, see this [talk](https://www.youtube.com/watch?v=SHaAR7XPtNU).
///
/// # Performance
/// It's performance characteristic are such that get/erase/insert are O(1) operations. **Unless**
/// inserting must allocate room for a new slot which then it becomes O(n).
#[derive(Debug)]
pub struct DenseSlotMap<T: Send + Sync> {
    /// Internal slots that act as a "pointer" between data and external keys
    indices: Vec<Slot<T>>,

    /// Holds the underlying data
    data: Vec<T>,

    /// References indices to handles that map to the data
    erase: Vec<usize>,

    /// A queue containing free indices
    free_queue: VecDeque<u64>,
}

impl<T: Send + Sync> Default for DenseSlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync> DenseSlotMap<T> {
    /// Create a new slot map. You should generally prefer to use [`DenseSlotMap::new_with_capacity`]
    /// as the largest advantage of slot maps is not having to constantly allocate new memory and
    /// re-using old memory.
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
            data: Vec::new(),
            erase: Vec::new(),
            free_queue: VecDeque::new(),
        }
    }

    /// Create a new slot map with a certain capacity
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            indices: Vec::with_capacity(capacity),
            data: Vec::with_capacity(capacity),
            erase: Vec::with_capacity(capacity),
            free_queue: VecDeque::with_capacity(capacity),
        }
    }

    /// Returns [`Ok`] if the slot is valid
    pub fn validate_slot(&self, slot: &Slot<T>) -> Result<()> {
        if let Some(index) = self.indices.get(slot.id() as usize) {
            if index.generation() != slot.generation() {
                return Err(anyhow::Error::from(
                    crate::error::DagalError::InvalidSlotMapSlot,
                ));
            }
        } else {
            return Err(anyhow::Error::from(
                crate::error::DagalError::InvalidSlotMapSlot,
            ));
        }
        Ok(())
    }

    /// Insert new data into the slot map
    pub fn insert(&mut self, data: T) -> Slot<T> {
        if self.free_queue.is_empty() {
            // no room, allocate more
            self.indices.push(Slot {
                id: self.data.len() as u64,
                generation: 0,
                _marker: Default::default(),
            });
            self.free_queue.push_front((self.indices.len() - 1) as u64);
        }
        let next_free_indices = self.free_queue.pop_back().unwrap();
        // generate a key to return back
        let key: &mut Slot<T> = self.indices.get_mut(next_free_indices as usize).unwrap();
        key.id = self.data.len() as u64;

        // create the data and update the indices index to point to it
        self.data.push(data);
        self.erase.push(next_free_indices as usize);
        let mut out_key = self
            .indices
            .get(next_free_indices as usize)
            .unwrap()
            .clone();
        out_key.id = next_free_indices;
        out_key
    }

    /// Erase data from the slot map effectively removing it entirely.
    ///
    /// **This only ensures that no new references are made to the data, but does not make
    /// checks regarding existing ones.**
    pub fn erase(&mut self, slot: Slot<T>) -> Result<T> {
        // validate generation
        self.validate_slot(&slot)?;
        // update generation to invalidate any future slots
        self.indices.get_mut(slot.id() as usize).unwrap().generation += 1;

        // swap data with the last data element and update the last data element's slot to its
        // new position
        let data_index = self.indices.get(slot.id as usize).unwrap().id as usize;
        let last_data_index = self.data.len() - 1;
        if data_index != last_data_index {
            self.data.swap(data_index, last_data_index);
            self.erase.swap(data_index, last_data_index);
            // update the original index
            let last_data_index = data_index; // since we swapped them
            let last_data_index_index = *self.erase.get(last_data_index).unwrap();
            self.indices.get_mut(last_data_index_index).unwrap().id = last_data_index as u64;
        }
        // remove the last bits
        let removed_data = self.data.pop().unwrap();
        self.erase.pop();

        self.free_queue.push_front(slot.id);
        Ok(removed_data)
    }

    /// Retrieve the data that maps to the slot directly
    pub fn get(&self, slot: &Slot<T>) -> Result<&T> {
        let data_index = self.get_data_index(slot)?;
        Ok(self.data.get(data_index as usize).unwrap())
    }

    /// Retrieve the data that maps to the slot directly as a mutable rw access
    pub fn get_mut(&mut self, slot: &Slot<T>) -> Result<&mut T> {
        let data_index = self.get_data_index(slot)?;
        Ok(self.data.get_mut(data_index as usize).unwrap())
    }

    /// Get a mutable iterator over all the data stored by the slot map.
    pub fn mut_iter_data(&mut self) -> IterMut<'_, T> {
        self.data.iter_mut()
    }

    /// Get the index the data is actually at in the `Data` vector
    pub fn get_data_index(&self, slot: &Slot<T>) -> Result<u64> {
        // validate generation
        self.validate_slot(slot)?;

        let data_index = self.indices.get(slot.id as usize).unwrap().id;
        Ok(data_index)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn create_retrieve_single() {
        // Test creating and retrieving a single item from a slot map
        let mut slot_map = crate::util::slot_map::DenseSlotMap::new();
        let handle = slot_map.insert(1);
        assert_eq!(1, *slot_map.get(&handle).unwrap());
    }

    #[test]
    fn create_retrieve_multiple() {
        // Test creating and retrieving a multiple items from a slot map
        let mut slot_map: crate::util::slot_map::DenseSlotMap<u32> =
            crate::util::slot_map::DenseSlotMap::new();
        let mut handles: Vec<crate::util::slot_map::Slot<u32>> = Vec::with_capacity(10);
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
        let mut slot_map: crate::util::slot_map::DenseSlotMap<u32> =
            crate::util::slot_map::DenseSlotMap::new();
        let mut handles: Vec<crate::util::slot_map::Slot<u32>> = Vec::with_capacity(10);
        for i in 0u32..10u32 {
            handles.push(slot_map.insert(i));
        }
        // Remove numbers which are even
        for i in 0u32..10u32 {
            if i % 2 == 0 {
                slot_map.erase(*handles.get(i as usize).unwrap()).unwrap();
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

    #[test]
    fn newest_item_is_last() {
        // We're testing to ensure that the newest data added will always be last
        let mut slot_map = crate::util::slot_map::DenseSlotMap::new();
        let handle = slot_map.insert(1);
        assert_eq!(1, *slot_map.get(&handle).unwrap());
        let handle = slot_map.insert(2);
        assert_eq!(*slot_map.data.last().unwrap(), 2i32);
    }

    #[test]
    fn swap_correct() {
        // We're testing to see swapping data correctly
        let mut slot_map = crate::util::slot_map::DenseSlotMap::new();
        let handle = slot_map.insert(1);
        assert_eq!(1, *slot_map.get(&handle).unwrap());
        let handle = slot_map.insert(2);
        let handle_gone = slot_map.insert(3);
        let handle = slot_map.insert(4);
        slot_map.erase(handle_gone).unwrap();
        assert_eq!(*slot_map.data.last().unwrap(), 4i32);
    }
}
