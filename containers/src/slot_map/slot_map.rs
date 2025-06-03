use crate::error::ContainerErrors;
use crate::prelude::DefaultSlot;
use std::slice::{Iter, IterMut};

/// Regular slot map implementation
#[derive(Debug, PartialEq, Eq)]
pub struct SlotMap<T> {
    // usize is a reference to the proxy slot index
    pub(crate) data: Vec<(T, u64)>,
    pub(crate) slots: Vec<DefaultSlot<T>>,
    pub(crate) free_list: Vec<u64>,
}
impl<T> Default for SlotMap<T> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            slots: Default::default(),
            free_list: Default::default(),
        }
    }
}

impl<T> SlotMap<T> {
    pub fn insert(&mut self, element: T) -> DefaultSlot<T> {
        // find the next free slot for indirect
        let free_slot_index;
        let free_slot: &mut DefaultSlot<T> = if let Some(index) = self.free_list.pop() {
            free_slot_index = index;
            self.slots.get_mut(index as usize).unwrap()
        } else {
            let slot = DefaultSlot::new(0, 0);
            free_slot_index = self.slots.len() as u64;
            self.slots.push(slot);
            self.slots.last_mut().unwrap()
        };
        // update index the inner slot will point to
        free_slot.id = self.data.len() as u64;
        // push data into data vec
        self.data.push((element, free_slot_index));

        // produce and out slot from mapping to the proxy slot

        DefaultSlot::new(free_slot_index, free_slot.generation)
    }

    pub fn remove(&mut self, slot: DefaultSlot<T>) -> Result<T, ContainerErrors> {
        if let Some(proxy_slot) = self.slots.get_mut(slot.id as usize).map(|proxy_slot| {
            if slot.generation != proxy_slot.generation {
                return Err(ContainerErrors::GenerationMismatch);
            }
            // increment generation
            proxy_slot.generation += 1;
            Ok::<DefaultSlot<T>, ContainerErrors>(proxy_slot.clone())
        }) {
            let proxy_slot = proxy_slot?;
            // swap (if needed) data before popping
            if !self.data.is_empty() && proxy_slot.id != (self.data.len() - 1) as u64 {
                let proxy_slot_data_index = proxy_slot.id;
                let last_index = self.data.len() - 1;
                // swap with the last
                self.data.swap(last_index, proxy_slot_data_index as usize);
                // update the indirect slot
                let swapped_proxy = self.data.get(proxy_slot_data_index as usize).unwrap().1;
                // since we swapped, we must update to the indirect to point to the data index
                self.slots
                    .get_mut(swapped_proxy as usize)
                    .map(|slot| slot.id = proxy_slot_data_index);
            }
            // to be removed must be last in data and slots
            let data = self.data.pop().unwrap();
            self.free_list.push(slot.id);
            Ok(data.0)
        } else {
            Err(ContainerErrors::NonexistentSlot)
        }
    }

    pub fn get(&self, slot: DefaultSlot<T>) -> Option<&T> {
        self.slots.get(slot.id as usize).and_then(|proxy_slot| {
            if proxy_slot.generation == slot.generation {
                self.data.get(proxy_slot.id as usize).map(|data| &data.0)
            } else {
                None
            }
        })
    }

    pub fn get_mut(&mut self, slot: DefaultSlot<T>) -> Option<&mut T> {
        self.slots.get(slot.id as usize).and_then(|proxy_slot| {
            if proxy_slot.generation == slot.generation {
                self.data
                    .get_mut(proxy_slot.id as usize)
                    .map(|data| &mut data.0)
            } else {
                None
            }
        })
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
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        assert_eq!(slot_map.get(slot), Some(&42));
    }

    #[test]
    fn test_insert_multiple_and_get() {
        let mut slot_map = SlotMap::default();
        let slot1 = slot_map.insert(42);
        let slot2 = slot_map.insert(43);
        let slot3 = slot_map.insert(44);

        assert_eq!(slot_map.get(slot1), Some(&42));
        assert_eq!(slot_map.get(slot2), Some(&43));
        assert_eq!(slot_map.get(slot3), Some(&44));
    }

    #[test]
    fn test_remove() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        let removed = slot_map.remove(slot.clone()).unwrap();
        assert_eq!(removed, 42);
        assert_eq!(slot_map.get(slot), None);
    }

    #[test]
    fn test_remove_and_insert() {
        let mut slot_map = SlotMap::default();
        let slot1 = slot_map.insert(42);
        let slot2 = slot_map.insert(43);
        let _ = slot_map.remove(slot1.clone()).unwrap();
        let slot3 = slot_map.insert(44);

        // Since slot1 was removed, slot3 may reuse that slot
        assert_eq!(slot3.id, slot1.id);
        assert_eq!(slot3.generation, slot1.generation + 1);
        assert_eq!(slot_map.get(slot2), Some(&43));
        assert_eq!(slot_map.get(slot3), Some(&44));
    }

    #[test]
    fn test_generation_mismatch() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
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
    fn test_nonexistent_slot() {
        let mut slot_map: SlotMap<i32> = SlotMap::default();
        let invalid_slot = DefaultSlot::new(999, 0); // Assuming we have less than 999 slots
        match slot_map.remove(invalid_slot) {
            Err(ContainerErrors::NonexistentSlot) => {}
            _ => panic!("Expected NonexistentSlot error"),
        }
    }

    #[test]
    fn test_get_mut() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        if let Some(value) = slot_map.get_mut(slot.clone()) {
            *value = 100;
        }
        assert_eq!(slot_map.get(slot), Some(&100));
    }

    #[test]
    fn test_iter() {
        let mut slot_map = SlotMap::default();
        let _ = slot_map.insert(1);
        let _ = slot_map.insert(2);
        let _ = slot_map.insert(3);

        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_iter_mut() {
        let mut slot_map = SlotMap::default();
        let _ = slot_map.insert(1);
        let _ = slot_map.insert(2);
        let _ = slot_map.insert(3);

        for (value, _) in slot_map.iter_mut() {
            *value *= 2;
        }

        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![2, 4, 6]);
    }

    #[test]
    fn test_large_number_of_elements() {
        let mut slot_map = SlotMap::default();
        let num_elements = 1000;
        let mut slots = Vec::new();

        for i in 0..num_elements {
            let slot = slot_map.insert(i);
            slots.push(slot);
        }

        for i in 0..num_elements {
            assert_eq!(slot_map.get(slots[i].clone()), Some(&i));
        }

        // Remove half of the elements
        for i in (0..num_elements).step_by(2) {
            let _ = slot_map.remove(slots[i].clone()).unwrap();
        }

        // Ensure removed elements are gone
        for i in (0..num_elements).step_by(2) {
            assert_eq!(slot_map.get(slots[i].clone()), None);
        }

        // Ensure remaining elements are still accessible
        for i in (1..num_elements).step_by(2) {
            assert_eq!(slot_map.get(slots[i].clone()), Some(&i));
        }
    }

    #[test]
    fn test_reuse_of_slots() {
        let mut slot_map = SlotMap::default();
        let slot1 = slot_map.insert(1);
        let slot2 = slot_map.insert(2);
        let slot3 = slot_map.insert(3);

        // Remove slot2
        let _ = slot_map.remove(slot2.clone()).unwrap();

        // Insert new element, which should reuse slot2's position
        let slot4 = slot_map.insert(4);

        // slot4 should have the same id as slot2 but with incremented generation
        assert_eq!(slot4.id, slot2.id);
        assert_eq!(slot4.generation, slot2.generation + 1);

        // Verify contents
        assert_eq!(slot_map.get(slot1), Some(&1));
        assert_eq!(slot_map.get(slot3), Some(&3));
        assert_eq!(slot_map.get(slot4), Some(&4));
    }

    #[test]
    fn test_remove_nonexistent_slot() {
        let mut slot_map: SlotMap<u64> = SlotMap::default();
        let slot = DefaultSlot::new(0, 0);
        match slot_map.remove(slot) {
            Err(ContainerErrors::NonexistentSlot) => {}
            _ => panic!("Expected NonexistentSlot error"),
        }
    }

    #[test]
    fn test_remove_invalid_generation() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        let invalid_slot = DefaultSlot::new(slot.id, slot.generation + 1);
        match slot_map.remove(invalid_slot) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_double_remove() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        let _ = slot_map.remove(slot.clone()).unwrap();
        // Try to remove again
        match slot_map.remove(slot) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_get_after_remove() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(42);
        let _ = slot_map.remove(slot.clone()).unwrap();
        assert_eq!(slot_map.get(slot), None);
    }

    #[test]
    fn test_insert_after_remove() {
        let mut slot_map = SlotMap::default();
        let slot1 = slot_map.insert(42);
        let _ = slot_map.remove(slot1.clone()).unwrap();
        let slot2 = slot_map.insert(43);

        // slot2 should have the same id as slot1 but incremented generation
        assert_eq!(slot2.id, slot1.id);
        assert_eq!(slot2.generation, slot1.generation + 1);

        assert_eq!(slot_map.get(slot2), Some(&43));
        assert_eq!(slot_map.get(slot1), None);
    }

    #[test]
    fn test_remove_all_and_insert() {
        let mut slot_map = SlotMap::default();
        let slot1 = slot_map.insert(1);
        let slot2 = slot_map.insert(2);

        let _ = slot_map.remove(slot1.clone()).unwrap();
        let _ = slot_map.remove(slot2.clone()).unwrap();

        let slot3 = slot_map.insert(3);
        let slot4 = slot_map.insert(4);

        // Since slots are reused, slot3 and slot4 may have same ids as slot1 and slot2
        assert_eq!(slot3.id, slot2.id);
        assert_eq!(slot4.id, slot1.id);

        // Generations should have incremented
        assert_eq!(slot3.generation, slot2.generation + 1);
        assert_eq!(slot4.generation, slot1.generation + 1);

        assert_eq!(slot_map.get(slot3), Some(&3));
        assert_eq!(slot_map.get(slot4), Some(&4));
    }

    #[test]
    fn test_insert_and_get_strings() {
        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(String::from("Hello"));
        assert_eq!(slot_map.get(slot), Some(&String::from("Hello")));
    }

    #[test]
    fn test_insert_and_get_custom_type() {
        #[derive(Debug, PartialEq)]
        struct Point {
            x: i32,
            y: i32,
        }

        let mut slot_map = SlotMap::default();
        let slot = slot_map.insert(Point { x: 1, y: 2 });
        assert_eq!(slot_map.get(slot), Some(&Point { x: 1, y: 2 }));
    }

    #[test]
    fn test_empty_slot_map() {
        let slot_map: SlotMap<i32> = SlotMap::default();
        assert_eq!(slot_map.data.len(), 0);
        assert_eq!(slot_map.slots.len(), 0);
        assert_eq!(slot_map.free_list.len(), 0);
    }
}
