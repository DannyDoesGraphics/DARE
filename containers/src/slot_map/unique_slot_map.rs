use crate::error::ContainerErrors;
use crate::prelude::DefaultSlot;
use crate::slot::{Slot, SlotWithGeneration};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::slice::{Iter, IterMut};

/// Unique slot map implementation that prevents duplicate values
#[derive(Debug)]
pub struct UniqueSlotMap<T, S: Slot + SlotWithGeneration = DefaultSlot<T>>
where
    T: Hash + Eq,
{
    // usize is a reference to the proxy slot index
    pub(crate) data: Vec<(T, u64)>,
    pub(crate) slots: Vec<S>,
    pub(crate) free_list: Vec<u64>,
    // HashMap to track existing value hashes and their slots for duplicate prevention
    pub(crate) hash_to_slot: HashMap<u64, S>,
}

impl<T, S: Slot + SlotWithGeneration> Default for UniqueSlotMap<T, S>
where
    T: Hash + Eq,
{
    fn default() -> Self {
        Self {
            data: Default::default(),
            slots: Default::default(),
            free_list: Default::default(),
            hash_to_slot: Default::default(),
        }
    }
}

impl<T, S: Slot + SlotWithGeneration> PartialEq for UniqueSlotMap<T, S>
where
    T: Hash + Eq + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.slots == other.slots && self.free_list == other.free_list
    }
}

impl<T, S: Slot + SlotWithGeneration> Eq for UniqueSlotMap<T, S> where T: Hash + Eq + PartialEq {}

impl<T, S: Slot + SlotWithGeneration> UniqueSlotMap<T, S>
where
    T: Hash + Eq,
{
    fn compute_hash(value: &T) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn is_duplicate(&self, element: &T) -> bool {
        let hash = Self::compute_hash(element);
        if let Some(existing_slot) = self.hash_to_slot.get(&hash) {
            // Hash collision check: verify the actual value matches
            if let Some(existing_value) = self.get(existing_slot.clone()) {
                return existing_value == element;
            }
        }
        false
    }

    pub fn new() -> Self {
        Self {
            data: Default::default(),
            slots: Default::default(),
            free_list: Default::default(),
            hash_to_slot: Default::default(),
        }
    }

    pub fn insert(&mut self, element: T) -> Result<S, ContainerErrors> {
        // Check for duplicates first
        if self.is_duplicate(&element) {
            return Err(ContainerErrors::DuplicateValue);
        }

        // find the next free slot for indirect
        let free_slot_index;
        let free_slot: &mut S = if let Some(index) = self.free_list.pop() {
            free_slot_index = index;
            self.slots.get_mut(index as usize).unwrap()
        } else {
            let slot: S = S::new_with_gen(0, 0);
            free_slot_index = self.slots.len() as u64;
            self.slots.push(slot);
            self.slots.last_mut().unwrap()
        };
        // update index the inner slot will point to
        free_slot.set_id(self.data.len() as u64);

        // Create the output slot
        let output_slot = S::new_with_gen(free_slot_index, free_slot.generation());

        // Add to hash tracking map (no cloning needed for the hash)
        let hash = Self::compute_hash(&element);
        self.hash_to_slot.insert(hash, output_slot.clone());

        // push data into data vec
        self.data.push((element, free_slot_index));

        Ok(output_slot)
    }

    /// Insert a value or return, if it exists.
    pub fn insert_or_get(&mut self, element: T) -> S {
        if let Some(existing_slot) = self.get_slot_for_value(&element) {
            return existing_slot.clone();
        }

        self.insert(element).unwrap()
    }

    /// Removes a slot from the hashmap
    pub fn remove(&mut self, slot: DefaultSlot<T>) -> Result<T, ContainerErrors> {
        if let Some(proxy_slot) = self.slots.get_mut(slot.id as usize).map(|proxy_slot| {
            if slot.generation != proxy_slot.generation() {
                return Err(ContainerErrors::GenerationMismatch);
            }
            // increment generation
            proxy_slot.set_generation(proxy_slot.generation() + 1);
            Ok::<S, ContainerErrors>(proxy_slot.clone())
        }) {
            let proxy_slot = proxy_slot?;
            // swap (if needed) data before popping
            if !self.data.is_empty() && proxy_slot.id() != (self.data.len() - 1) as u64 {
                let proxy_slot_data_index = proxy_slot.id();
                let last_index = self.data.len() - 1;
                // swap with the last
                self.data.swap(last_index, proxy_slot_data_index as usize);
                // update the indirect slot
                let swapped_proxy = self.data.get(proxy_slot_data_index as usize).unwrap().1;
                // since we swapped, we must update to the indirect to point to the data index
                if let Some(slot) = self.slots.get_mut(swapped_proxy as usize) {
                    slot.set_id(proxy_slot_data_index)
                }
            }
            // to be removed must be last in data and slots
            let data = self.data.pop().unwrap();

            // Remove from hash tracking map
            let hash = Self::compute_hash(&data.0);
            self.hash_to_slot.remove(&hash);

            self.free_list.push(slot.id);
            Ok(data.0)
        } else {
            Err(ContainerErrors::NonexistentSlot)
        }
    }

    pub fn get(&self, slot: S) -> Option<&T> {
        self.slots.get(slot.id() as usize).and_then(|proxy_slot| {
            if proxy_slot.generation() == slot.generation() {
                self.data.get(proxy_slot.id() as usize).map(|data| &data.0)
            } else {
                None
            }
        })
    }

    pub fn get_mut(&mut self, slot: DefaultSlot<T>) -> Option<&mut T> {
        self.slots.get(slot.id as usize).and_then(|proxy_slot| {
            if proxy_slot.generation() == slot.generation() {
                self.data
                    .get_mut(proxy_slot.id() as usize)
                    .map(|data| &mut data.0)
            } else {
                None
            }
        })
    }

    pub fn contains_value(&self, value: &T) -> bool {
        self.is_duplicate(value)
    }

    pub fn get_slot_for_value(&self, value: &T) -> Option<&S> {
        let hash = Self::compute_hash(value);
        if let Some(slot) = self.hash_to_slot.get(&hash) {
            // Verify it's actually the same value (handle hash collisions)
            if let Some(existing_value) = self.get(slot.clone()) {
                if existing_value == value {
                    return Some(slot);
                }
            }
        }
        None
    }

    pub fn iter(&self) -> Iter<'_, (T, u64)> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<(T, u64)> {
        self.data.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut slot_map = UniqueSlotMap::default();
        let slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        assert_eq!(slot_map.get(slot), Some(&42));
    }

    #[test]
    fn test_insert_duplicate_fails() {
        let mut slot_map = UniqueSlotMap::default();
        let _slot1: DefaultSlot<i32> = slot_map.insert(42).unwrap();

        // Try to insert the same value again
        match slot_map.insert(42) {
            Err(ContainerErrors::DuplicateValue) => {}
            _ => panic!("Expected DuplicateValue error"),
        }
    }

    #[test]
    fn test_insert_multiple_unique_values() {
        let mut slot_map = UniqueSlotMap::default();
        let slot1: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        let slot2 = slot_map.insert(43).unwrap();
        let slot3 = slot_map.insert(44).unwrap();

        assert_eq!(slot_map.get(slot1), Some(&42));
        assert_eq!(slot_map.get(slot2), Some(&43));
        assert_eq!(slot_map.get(slot3), Some(&44));
    }

    #[test]
    fn test_remove_and_reinsert() {
        let mut slot_map = UniqueSlotMap::default();
        let slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        let removed = slot_map.remove(slot.clone()).unwrap();
        assert_eq!(removed, 42);
        assert_eq!(slot_map.get(slot), None);

        // Now we should be able to insert 42 again
        let new_slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        assert_eq!(slot_map.get(new_slot), Some(&42));
    }

    #[test]
    fn test_contains_value() {
        let mut slot_map = UniqueSlotMap::default();
        assert!(!slot_map.contains_value(&42));

        let _slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        assert!(slot_map.contains_value(&42));
        assert!(!slot_map.contains_value(&43));
    }

    #[test]
    fn test_get_slot_for_value() {
        let mut slot_map = UniqueSlotMap::default();
        let slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();

        let found_slot = slot_map.get_slot_for_value(&42);
        assert_eq!(found_slot, Some(&slot));

        let not_found = slot_map.get_slot_for_value(&43);
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_insert_and_get_strings() {
        let mut slot_map = UniqueSlotMap::default();
        let slot: DefaultSlot<String> = slot_map.insert(String::from("Hello")).unwrap();
        assert_eq!(slot_map.get(slot), Some(&String::from("Hello")));

        // Try to insert duplicate string
        match slot_map.insert(String::from("Hello")) {
            Err(ContainerErrors::DuplicateValue) => {}
            _ => panic!("Expected DuplicateValue error"),
        }
    }

    #[test]
    fn test_insert_and_get_custom_type() {
        #[derive(Debug, PartialEq, Eq, Hash, Clone)]
        struct Point {
            x: i32,
            y: i32,
        }

        let mut slot_map = UniqueSlotMap::default();
        let point = Point { x: 1, y: 2 };
        let slot: DefaultSlot<Point> = slot_map.insert(point.clone()).unwrap();
        assert_eq!(slot_map.get(slot), Some(&point));

        // Try to insert duplicate point
        match slot_map.insert(point) {
            Err(ContainerErrors::DuplicateValue) => {}
            _ => panic!("Expected DuplicateValue error"),
        }
    }
    #[test]
    fn test_empty_slot_map() {
        let slot_map: UniqueSlotMap<i32> = UniqueSlotMap::default();
        assert_eq!(slot_map.data.len(), 0);
        assert_eq!(slot_map.slots.len(), 0);
        assert_eq!(slot_map.free_list.len(), 0);
        assert_eq!(slot_map.hash_to_slot.len(), 0);
    }

    #[test]
    fn test_generation_mismatch() {
        let mut slot_map = UniqueSlotMap::default();
        let slot: DefaultSlot<i32> = slot_map.insert(42).unwrap();
        // Remove the slot
        let _ = slot_map.remove(slot.clone()).unwrap();
        // Try to get or remove using the same slot (should fail due to generation mismatch)
        assert_eq!(slot_map.get(slot.clone()), None);
        match slot_map.remove(slot.clone()) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_reuse_of_slots_after_remove() {
        let mut slot_map = UniqueSlotMap::default();
        let slot1: DefaultSlot<i32> = slot_map.insert(1).unwrap();
        let slot2 = slot_map.insert(2).unwrap();
        let slot3 = slot_map.insert(3).unwrap();

        // Remove slot2
        let _ = slot_map.remove(slot2.clone()).unwrap();

        // Insert new element, which should reuse slot2's position
        let slot4 = slot_map.insert(4).unwrap();

        // slot4 should have the same id as slot2 but with incremented generation
        assert_eq!(slot4.id, slot2.id);
        assert_eq!(slot4.generation, slot2.generation + 1);

        // Verify contents
        assert_eq!(slot_map.get(slot1), Some(&1));
        assert_eq!(slot_map.get(slot3), Some(&3));
        assert_eq!(slot_map.get(slot4), Some(&4));
    }
    #[test]
    fn test_complex_duplicate_scenario() {
        let mut slot_map = UniqueSlotMap::default();

        // Insert some values
        let _slot1: DefaultSlot<i32> = slot_map.insert(10).unwrap();
        let slot2 = slot_map.insert(20).unwrap();
        let _slot3 = slot_map.insert(30).unwrap();

        // Try to insert duplicates - should all fail
        assert!(slot_map.insert(10).is_err());
        assert!(slot_map.insert(20).is_err());
        assert!(slot_map.insert(30).is_err());

        // Remove one value
        let _ = slot_map.remove(slot2).unwrap();

        // Now we should be able to insert 20 again
        let new_slot = slot_map.insert(20).unwrap();
        assert_eq!(slot_map.get(new_slot), Some(&20));

        // But other duplicates should still fail
        assert!(slot_map.insert(10).is_err());
        assert!(slot_map.insert(30).is_err());
    }
    #[test]
    fn test_insert_or_get() {
        let mut slot_map = UniqueSlotMap::default();

        // First insertion should create new slot
        let slot1: DefaultSlot<i32> = slot_map.insert_or_get(42);
        assert_eq!(slot_map.get(slot1.clone()), Some(&42));

        // Second insertion of same value should return same slot
        let slot2 = slot_map.insert_or_get(42);
        assert_eq!(slot1, slot2);
        assert_eq!(slot_map.data.len(), 1); // Only one element stored

        // Different value should create new slot
        let slot3 = slot_map.insert_or_get(100);
        assert_ne!(slot1, slot3);
        assert_eq!(slot_map.get(slot3), Some(&100));
        assert_eq!(slot_map.data.len(), 2); // Two elements stored
    }

    #[test]
    fn test_entry() {
        let mut slot_map = UniqueSlotMap::default();

        // First call should create new slot
        let slot1: DefaultSlot<String> = slot_map.insert_or_get(String::from("hello"));
        assert_eq!(slot_map.get(slot1.clone()), Some(&String::from("hello")));

        // Second call with same value should return same slot
        let slot2 = slot_map.insert_or_get(String::from("hello"));
        assert_eq!(slot1, slot2);
        assert_eq!(slot_map.data.len(), 1); // Only one element stored
    }

    #[test]
    fn test_entry_after_remove_and_reinsert() {
        let mut slot_map = UniqueSlotMap::default();

        // Insert value
        let slot1: DefaultSlot<i32> = slot_map.insert_or_get(42);
        assert_eq!(slot_map.get(slot1.clone()), Some(&42));

        // Remove value
        let removed = slot_map.remove(slot1.clone()).unwrap();
        assert_eq!(removed, 42);

        // Entry should create new slot (different from slot1 due to generation)
        let slot2 = slot_map.insert_or_get(42);
        assert_ne!(slot1, slot2); // Different due to generation increment
        assert_eq!(slot_map.get(slot2), Some(&42));
        assert_eq!(slot_map.get(slot1), None); // Old slot should be invalid
    }

    #[test]
    fn test_entry_vs_insert_consistency() {
        let mut slot_map1: UniqueSlotMap<i32> = UniqueSlotMap::default();
        let mut slot_map2: UniqueSlotMap<i32> = UniqueSlotMap::default();

        // Use entry() on first map
        let slot1: DefaultSlot<i32> = slot_map1.insert_or_get(100);

        // Use insert() on second map
        let slot2: DefaultSlot<i32> = slot_map2.insert(100).unwrap();

        // Both should have same generation and id (0)
        assert_eq!(slot1.id, slot2.id);
        assert_eq!(slot1.generation, slot2.generation);
        assert_eq!(slot_map1.get(slot1), slot_map2.get(slot2));
    }
}
