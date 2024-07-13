use anyhow::Result;

use crate::DagalError;
use crate::util::slot_map::Slot;
use crate::util::traits::SlotMap;

#[derive(Debug, Copy, Clone, Default)]
pub struct SlotEntry<T> {
    pub data: Option<T>,
    pub slot: Slot<T>,
}

impl<T> PartialEq for SlotEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
    }
}

/// tl;dr Works much more similar to a [`FreeList`](crate::util::FreeList) with generation counters.
///
/// A SparseSlotMap is a slot map where it does not attempt to dense pack all the data together.
/// When data is deleted, it leaves a gap in the vector and notes that it is free similar to a FreeList.
/// This means we can sacrifice the indices vector and have direct handle mappings to the data in the
/// data vector.
///
/// # Performance characteristics
/// O(1) insertions/deletion
///
/// 1 level of indirection due to direct handle mappings to the underlying data's location
///
/// Faster deletion time as no data swaps must occur
#[derive(Debug)]
pub struct SparseSlotMap<T> {
    /// Store the data right next to it's handle
    data: Vec<SlotEntry<T>>,
    /// List of freed slots
    free_list: Vec<usize>,
}

impl<T> Default for SparseSlotMap<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            free_list: Vec::new(),
        }
    }
}

impl<T> SparseSlotMap<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            free_list: Vec::new(),
        }
    }

    /// Insert an element into a sparse slot map
    pub fn insert(&mut self, data: T) -> Slot<T> {
        let next_free_index = self.free_list.pop().unwrap_or_else(|| {
            self.data.push(SlotEntry {
                data: None,
                slot: Slot::new(self.data.len() as u64, None),
            });
            self.data.len() - 1
        });
        let slot = self.data.get_mut(next_free_index).unwrap();
        slot.data = Some(data);

        slot.slot.clone()
    }

    /// Remove an element from a SparseSlotMap by slot
    pub fn remove(&mut self, slot: Slot<T>) -> Result<T> {
        if !self.is_valid_slot(&slot) {
            return Err(anyhow::Error::from(DagalError::InvalidSlotMapSlot));
        }
        let slot_union = self.data.get_mut(slot.id as usize).unwrap();
        slot_union.slot.generation += 1; // invalidate
        Ok(slot_union.data.take().unwrap())
    }

    /// Checks if a given slot is valid in the SparseSlotMap
    pub fn is_valid_slot(&self, slot: &Slot<T>) -> bool {
        return self
            .data
            .get(slot.id as usize)
            .map(|slot_union| *slot == slot_union.slot && slot_union.data.is_some())
            .unwrap_or(false);
    }

    /// Count # of used slots
    pub fn count_used(&self) -> usize {
        self.data.iter().filter(|data| data.data.is_some()).count()
    }

    pub fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Slot<T>, f: F) -> Result<R> {
        if !self.is_valid_slot(slot) {
            return Err(anyhow::Error::from(DagalError::InvalidSlotMapSlot));
        }
        Ok(f(self
            .data
            .get(slot.id() as usize)
            .as_ref()
            .unwrap()
            .data
            .as_ref()
            .unwrap()))
    }

    pub fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(
        &mut self,
        slot: &mut Slot<T>,
        f: F,
    ) -> Result<R> {
        if !self.is_valid_slot(slot) {
            return Err(anyhow::Error::from(DagalError::InvalidSlotMapSlot));
        }
        Ok(f(self
            .data
            .get_mut(slot.id() as usize)
            .unwrap()
            .data
            .as_mut()
            .unwrap()))
    }

    pub fn get_data_len(&self) -> usize {
        self.data.len()
    }

    pub fn iter(&self) -> impl Iterator<Item=&SlotEntry<T>> {
        self.data.iter().enumerate().map(|(index, entry)| {
            entry
        })
    }

    pub fn data_mut(&mut self) -> &mut Vec<SlotEntry<T>> {
        &mut self.data
    }

    pub fn data(&self) -> &[SlotEntry<T>] {
        self.data.as_slice()
    }
}

/**
impl<T> SlotMap<T> for SparseSlotMap<T> {
    type Slot = SlotEntry<T>;

    fn get_data(&self) -> &[Self::Slot] {
        self.data.as_slice()
    }

    fn all_slot_len(&self) -> usize {
        self.data.len()
    }

    fn is_valid_slot(&self, slot: &Slot<T>) -> bool {
        self
            .data
            .get(slot.id as usize)
            .map(|slot_union| *slot == slot_union.slot && slot_union.data.is_some())
            .unwrap_or(false)
    }

    fn insert(&mut self, element: T) -> Slot<T> {
        todo!()
    }

    fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Slot<T>, func: F) -> Result<R> {
        todo!()
    }

    fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(&mut self, slot: &Slot<T>, func: F) -> Result<R> {
        todo!()
    }
}
 **/
#[cfg(test)]
mod tests {
    use crate::util::slot_map::Slot;

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestData {
        value: i32,
    }

    #[test]
    fn test_insert() {
        let mut map = SparseSlotMap::new(10);
        let data = TestData { value: 42 };

        let slot = map.insert(data.clone());

        assert!(map.is_valid_slot(&slot));
        assert_eq!(map.data[slot.id as usize].data, Some(data));
    }

    #[test]
    fn test_remove() {
        let mut map = SparseSlotMap::new(10);
        let data = TestData { value: 42 };

        let slot = map.insert(data.clone());
        let removed_data = map.remove(slot.clone()).expect("Failed to remove data");

        assert_eq!(removed_data, data);
        assert!(!map.is_valid_slot(&slot));
        assert!(map.data[slot.id as usize].data.is_none());
    }

    #[test]
    fn test_is_valid_slot() {
        let mut map = SparseSlotMap::new(10);
        let data = TestData { value: 42 };

        let slot = map.insert(data);
        assert!(map.is_valid_slot(&slot));

        let invalid_slot = Slot::new(999, None); // An invalid slot
        assert!(!map.is_valid_slot(&invalid_slot));

        let removed_slot = map.remove(slot.clone()).expect("Failed to remove data");
        assert!(!map.is_valid_slot(&slot));
    }

    #[test]
    fn test_reuse_slots() {
        let mut map = SparseSlotMap::new(10);
        let data1 = TestData { value: 42 };
        let data2 = TestData { value: 43 };

        let slot1 = map.insert(data1);
        map.remove(slot1.clone()).expect("Failed to remove data");

        let slot2 = map.insert(data2.clone());
        assert_eq!(slot1.id, slot2.id); // Slot should be reused
        assert!(map.is_valid_slot(&slot2));
        assert_eq!(map.data[slot2.id as usize].data, Some(data2));
    }

    #[test]
    fn test_remove_invalid_slot() {
        let mut map: SparseSlotMap<TestData> = SparseSlotMap::new(10);
        let invalid_slot = Slot::new(999, None); // An invalid slot

        let result = map.remove(invalid_slot);
        assert!(result.is_err());
    }
}
