use crate::error::ContainerErrors;
use crate::prelude::Slot;
use std::ops::{Deref, DerefMut};

/// Regular slot map implementation

#[derive(Debug, PartialEq, Eq)]
pub struct InsertionSortSlotMap<T: Eq + PartialEq + PartialOrd + Ord> {
    // usize is a reference to the proxy slot index
    pub(crate) handle: super::SlotMap<T>,
}

impl<T: Eq + PartialEq + PartialOrd + Ord> Default for InsertionSortSlotMap<T> {
    fn default() -> Self {
        Self {
            handle: super::SlotMap::default(),
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
        let position_in_vec = self
            .handle
            .data
            .binary_search_by(|(probe, _)| probe.cmp(&element))
            .unwrap_or_else(|e| e);

        // get next free slot
        let free_slot_index: usize = if let Some(index) = self.free_list.pop() {
            index
        } else {
            self.slots.push(Slot::new(0, 0));
            (self.slots.len() - 1) as u64
        } as usize;
        // update id
        self.slots[free_slot_index].id = position_in_vec as u64;

        self.data
            .insert(position_in_vec, (element, free_slot_index as u64));
        // update all mappings after
        let updates: Vec<(u64, usize)> = self
            .data
            .iter()
            .enumerate()
            .skip(position_in_vec + 1)
            .map(|(i, &(_, slot_index))| (slot_index, i))
            .collect();

        for (slot_index, new_id) in updates {
            self.slots[slot_index as usize].id = new_id as u64;
        }

        // produce and out slot from mapping to the proxy slot
        let out_slot = Slot::new(
            free_slot_index as u64,
            self.slots[free_slot_index].generation,
        );
        Ok(out_slot)
    }

    /// Removes a slot as according to insertion removal
    pub fn insertion_removal(&mut self, slot: Slot<T>) -> Result<T, ContainerErrors> {
        if let Some(proxy_slot) = self.slots.get_mut(slot.id as usize).map(|proxy_slot| {
            if slot.generation != proxy_slot.generation {
                return Err(ContainerErrors::GenerationMismatch);
            }
            // increment generation
            proxy_slot.generation += 1;
            Ok::<Slot<T>, ContainerErrors>(proxy_slot.clone())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insertion_sort_insert_and_get() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        assert_eq!(slot_map.get(slot), Some(&42));
        assert_eq!(slot_map.handle.data.len(), 1);
        assert_eq!(slot_map.handle.data[0].0, 42);
    }

    #[test]
    fn test_insertion_sort_insert_multiple_and_get_sorted() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(50).unwrap();
        let slot2 = slot_map.insertion_sort(30).unwrap();
        let slot3 = slot_map.insertion_sort(40).unwrap();

        // The data should be sorted: [30, 40, 50]
        let expected = vec![30, 40, 50];
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, expected);

        // Verify that slots can retrieve the correct values
        assert_eq!(slot_map.get(slot1), Some(&50));
        assert_eq!(slot_map.get(slot2), Some(&30));
        assert_eq!(slot_map.get(slot3), Some(&40));
    }

    #[test]
    fn test_insertion_sort_insert_duplicates() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(42).unwrap();
        let slot2 = slot_map.insertion_sort(42).unwrap();
        let slot3 = slot_map.insertion_sort(42).unwrap();

        // All values are 42, verify they are all inserted
        assert_eq!(slot_map.handle.data.len(), 3);
        let expected = vec![42, 42, 42];
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, expected);

        // Verify that slots retrieve the correct values
        assert_eq!(slot_map.get(slot1), Some(&42));
        assert_eq!(slot_map.get(slot2), Some(&42));
        assert_eq!(slot_map.get(slot3), Some(&42));
    }

    #[test]
    fn test_insertion_sort_remove() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(30).unwrap();
        let slot2 = slot_map.insertion_sort(20).unwrap();
        let slot3 = slot_map.insertion_sort(40).unwrap();

        // Data should be [20, 30, 40]
        let collected_before: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected_before, vec![20, 30, 40]);

        // Remove 30
        let removed = slot_map.remove(slot1.clone()).unwrap();
        assert_eq!(removed, 30);

        // Data should now be [20, 40]
        let collected_after: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected_after, vec![20, 40]);

        // Ensure slot1 cannot be used anymore
        assert_eq!(slot_map.get(slot1), None);

        // Verify other slots are still valid
        assert_eq!(slot_map.get(slot2), Some(&20));
        assert_eq!(slot_map.get(slot3), Some(&40));
    }

    #[test]
    fn test_insertion_sort_remove_and_insert() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(10).unwrap();
        let slot2 = slot_map.insertion_sort(20).unwrap();
        let slot3 = slot_map.insertion_sort(30).unwrap();

        // Remove slot2 (value 20)
        let _ = slot_map.remove(slot2.clone()).unwrap();

        // Insert new element 25
        let slot4 = slot_map.insertion_sort(25).unwrap();

        // Data should be [10, 25, 30]
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![10, 25, 30]);

        // Verify slots
        assert_eq!(slot_map.get(slot1), Some(&10));
        assert_eq!(slot_map.get(slot2), None); // Removed
        assert_eq!(slot_map.get(slot3), Some(&30));
        assert_eq!(slot_map.get(slot4), Some(&25));
    }

    #[test]
    fn test_insertion_sort_generation_mismatch() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        // Remove the slot
        let _ = slot_map.remove(slot.clone()).unwrap();
        // Try to get or remove using the same slot (should fail due to generation mismatch)
        assert_eq!(slot_map.get(slot.clone()), None);
        match slot_map.remove(slot) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_insertion_sort_nonexistent_slot() {
        let mut slot_map: InsertionSortSlotMap<i32> = InsertionSortSlotMap::default();
        let invalid_slot = Slot::new(999, 0); // Assuming we have less than 999 slots
        match slot_map.remove(invalid_slot) {
            Err(ContainerErrors::NonexistentSlot) => {}
            _ => panic!("Expected NonexistentSlot error"),
        }
    }

    #[test]
    fn test_insertion_sort_get_mut() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        if let Some(value) = slot_map.get_mut(slot.clone()) {
            *value = 100;
        }
        // Data should still be sorted
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![100]);

        assert_eq!(slot_map.get(slot), Some(&100));
    }

    #[test]
    fn test_insertion_sort_iter() {
        let mut slot_map = InsertionSortSlotMap::default();
        let _ = slot_map.insertion_sort(3).unwrap();
        let _ = slot_map.insertion_sort(1).unwrap();
        let _ = slot_map.insertion_sort(2).unwrap();

        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_insertion_sort_iter_mut() {
        let mut slot_map = InsertionSortSlotMap::default();
        let _ = slot_map.insertion_sort(1).unwrap();
        let _ = slot_map.insertion_sort(2).unwrap();
        let _ = slot_map.insertion_sort(3).unwrap();

        // Multiply each element by 2
        for (value, _) in slot_map.iter_mut() {
            *value *= 2;
        }

        // Data should still be sorted
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![2, 4, 6]);
    }

    #[test]
    fn test_insertion_sort_large_number_of_elements() {
        let mut slot_map = InsertionSortSlotMap::default();
        let num_elements = 1000;
        let mut slots = Vec::new();

        // Insert elements in reverse order to test sorting
        for i in (0..num_elements).rev() {
            let slot = slot_map.insertion_sort(i).unwrap();
            slots.push(slot);
        }

        // Verify that data is sorted
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        let expected: Vec<_> = (0..num_elements).collect();
        assert_eq!(collected, expected);

        // Remove half of the elements
        for i in (0..num_elements).step_by(2) {
            let _ = slot_map.remove(slots[i].clone()).unwrap();
        }

        // Ensure removed elements are gone
        for i in (0..num_elements).step_by(2) {
            assert_eq!(slot_map.get(slots[i].clone()), None);
        }

        // Ensure remaining elements are still accessible and sorted
        let collected_after: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        let expected_after: Vec<_> = (1..num_elements).step_by(2).collect();
        assert_eq!(collected_after, expected_after);
    }

    #[test]
    fn test_insertion_sort_reuse_of_slots() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(10).unwrap();
        let slot2 = slot_map.insertion_sort(20).unwrap();
        let slot3 = slot_map.insertion_sort(30).unwrap();

        // Remove slot2
        let _ = slot_map.remove(slot2.clone()).unwrap();

        // Insert new element, which should go between 10 and 30
        let slot4 = slot_map.insertion_sort(25).unwrap();

        // Verify that data is sorted: [10, 25, 30]
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![10, 25, 30]);

        // slot4 may reuse slot2's id with incremented generation
        assert_eq!(slot4.id, slot2.id);
        assert_eq!(slot4.generation, slot2.generation + 1);

        // Verify contents
        assert_eq!(slot_map.get(slot1), Some(&10));
        assert_eq!(slot_map.get(slot3), Some(&30));
        assert_eq!(slot_map.get(slot4), Some(&25));
    }

    #[test]
    fn test_insertion_sort_remove_nonexistent_slot() {
        let mut slot_map: InsertionSortSlotMap<usize> = InsertionSortSlotMap::default();
        let slot = Slot::new(0, 0);
        match slot_map.remove(slot) {
            Err(ContainerErrors::NonexistentSlot) => {}
            _ => panic!("Expected NonexistentSlot error"),
        }
    }

    #[test]
    fn test_insertion_sort_remove_invalid_generation() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        let invalid_slot = Slot::new(slot.id, slot.generation + 1);
        match slot_map.remove(invalid_slot) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_insertion_sort_double_remove() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        let _ = slot_map.remove(slot.clone()).unwrap();
        // Try to remove again
        match slot_map.remove(slot.clone()) {
            Err(ContainerErrors::GenerationMismatch) => {}
            _ => panic!("Expected GenerationMismatch error"),
        }
    }

    #[test]
    fn test_insertion_sort_get_after_remove() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(42).unwrap();
        let _ = slot_map.remove(slot.clone()).unwrap();
        assert_eq!(slot_map.get(slot.clone()), None);
    }

    #[test]
    fn test_insertion_sort_insert_after_remove() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(20).unwrap();
        let _ = slot_map.remove(slot1.clone()).unwrap();
        let slot2 = slot_map.insertion_sort(25).unwrap();

        // slot2 should have the same id as slot1 but incremented generation
        assert_eq!(slot2.id, slot1.id);
        assert_eq!(slot2.generation, slot1.generation + 1);

        assert_eq!(slot_map.get(slot2), Some(&25));
        assert_eq!(slot_map.get(slot1), None);
    }

    #[test]
    fn test_insertion_sort_remove_all_and_insert() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(10).unwrap();
        let slot2 = slot_map.insertion_sort(20).unwrap();

        let _ = slot_map.remove(slot1.clone()).unwrap();
        let _ = slot_map.remove(slot2.clone()).unwrap();

        let slot3 = slot_map.insertion_sort(15).unwrap();
        let slot4 = slot_map.insertion_sort(25).unwrap();

        // Data should be [15, 25]
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![15, 25]);

        // Since slots are reused, slot3 and slot4 may have same ids as slot1 and slot2
        assert_eq!(slot3.id, slot1.id);
        assert_eq!(slot4.id, slot2.id);

        // Generations should have incremented
        assert_eq!(slot3.generation, slot1.generation + 1);
        assert_eq!(slot4.generation, slot2.generation + 1);

        assert_eq!(slot_map.get(slot3), Some(&15));
        assert_eq!(slot_map.get(slot4), Some(&25));
    }

    #[test]
    fn test_insertion_sort_insert_and_get_strings() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot = slot_map.insertion_sort(String::from("banana")).unwrap();
        let slot2 = slot_map.insertion_sort(String::from("apple")).unwrap();
        let slot3 = slot_map.insertion_sort(String::from("cherry")).unwrap();

        // Data should be sorted alphabetically: ["apple", "banana", "cherry"]
        let expected = vec![
            String::from("apple"),
            String::from("banana"),
            String::from("cherry"),
        ];
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| value.clone()).collect();
        assert_eq!(collected, expected);

        // Verify that slots can retrieve the correct values
        assert_eq!(slot_map.get(slot), Some(&String::from("banana")));
        assert_eq!(slot_map.get(slot2), Some(&String::from("apple")));
        assert_eq!(slot_map.get(slot3), Some(&String::from("cherry")));
    }

    #[test]
    fn test_insertion_sort_insert_and_get_custom_type() {
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
        struct Point {
            x: i32,
            y: i32,
        }

        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(Point { x: 1, y: 2 }).unwrap();
        let slot2 = slot_map.insertion_sort(Point { x: 0, y: 3 }).unwrap();
        let slot3 = slot_map.insertion_sort(Point { x: 2, y: 1 }).unwrap();

        // Assuming Point implements Ord based on x and then y
        let expected = vec![
            Point { x: 0, y: 3 },
            Point { x: 1, y: 2 },
            Point { x: 2, y: 1 },
        ];
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| value.clone()).collect();
        assert_eq!(collected, expected);

        // Verify slots
        assert_eq!(slot_map.get(slot1), Some(&Point { x: 1, y: 2 }));
        assert_eq!(slot_map.get(slot2), Some(&Point { x: 0, y: 3 }));
        assert_eq!(slot_map.get(slot3), Some(&Point { x: 2, y: 1 }));
    }

    #[test]
    fn test_insertion_sort_empty_slot_map() {
        let slot_map: InsertionSortSlotMap<i32> = InsertionSortSlotMap::default();
        assert_eq!(slot_map.data.len(), 0);
        assert_eq!(slot_map.slots.len(), 0);
        assert_eq!(slot_map.free_list.len(), 0);
    }

    #[test]
    fn test_insertion_sort_modify_element() {
        let mut slot_map = InsertionSortSlotMap::default();
        let slot1 = slot_map.insertion_sort(10).unwrap();
        let slot2 = slot_map.insertion_sort(20).unwrap();
        let slot3 = slot_map.insertion_sort(30).unwrap();

        // Modify the element at slot2 to 25
        if let Some(value) = slot_map.get_mut(slot2.clone()) {
            *value = 25;
        }

        // Data should still be sorted
        let collected: Vec<_> = slot_map.iter().map(|(value, _)| *value).collect();
        assert_eq!(collected, vec![10, 25, 30]);

        // Verify slots
        assert_eq!(slot_map.get(slot1), Some(&10));
        assert_eq!(slot_map.get(slot2), Some(&25));
        assert_eq!(slot_map.get(slot3), Some(&30));
    }
}
