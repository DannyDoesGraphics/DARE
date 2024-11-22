use std::slice::{Iter, IterMut};
use crate::error::ContainerErrors;
use crate::prelude::Slot;

/// Regular slot map implementation
#[derive(Debug, PartialEq, Eq)]
pub struct SlotMap<T> {
    // usize is a reference to the proxy slot index
    pub(crate) data: Vec<(T, usize)>,
    pub(crate) slots: Vec<Slot<T>>,
    pub(crate) free_list: Vec<Slot<T>>,
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
    pub fn insert(&mut self, element: T) -> Result<Slot<T>, ContainerErrors> {
        // find the next free slot for indirect
        let mut free_slot = self.free_list.pop().unwrap_or_else(|| {
            let slot = Slot::new(self.data.len(), 0);
            self.slots.push(slot.clone());
            slot
        });
        // push data into data vec
        self.data.push((element, self.slots.len() - 1));

        // produce and out slot from mapping to the proxy slot
        let out_slot = Slot::new(
            self.slots.len() - 1,
            free_slot.generation
        );
        Ok(out_slot)
    }

    pub fn remove(&mut self, slot: Slot<T>) -> Result<T, ContainerErrors> {
        if let Some(mut proxy_slot) = self.slots.get(slot.id).map(|slot| slot.clone()) {
            if proxy_slot.generation == slot.generation {
                proxy_slot.generation += 1;
                // swap (if needed) data before popping
                if proxy_slot.id != self.data.len() - 1 {
                    let proxy_slot_data_index = proxy_slot.id;
                    let last_index = self.data.len() - 1;
                    // swap with the last
                    self.data.swap(last_index, proxy_slot_data_index);
                    // update the indirect slot
                    let swapped_proxy = self.data.get(proxy_slot_data_index).unwrap().1;
                    // since we swapped, we must update to the indirect to point to the data index
                    self.slots.get_mut(swapped_proxy).map(|slot| slot.id = proxy_slot_data_index);
                }
                // to be removed must be last in data and slots
                let data = self.data.pop().unwrap();
                self.free_list.push(proxy_slot.clone());
                Ok(data.0)
            } else {
                Err(ContainerErrors::GenerationMismatch)
            }
        } else {
            Err(ContainerErrors::NonexistentSlot)
        }
    }

    pub fn get(&self, slot: Slot<T>) -> Option<&T> {
        self.slots.get(slot.id).map(|proxy_slot| {
            if proxy_slot.generation == slot.generation {
                self.data.get(proxy_slot.id).map(|data| &data.0)
            } else {
                None
            }
        }).flatten()
    }

    pub fn get_mut(&mut self, slot: Slot<T>) -> Option<&mut T> {
        self.slots.get(slot.id).map(|proxy_slot| {
            if proxy_slot.generation == slot.generation {
                self.data.get_mut(proxy_slot.id).map(|data| &mut data.0)
            } else {
                None
            }
        }).flatten()
    }

    pub fn iter(&self) -> Iter<'_, (T, usize)> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<(T, usize)> {
        self.data.iter_mut()
    }
}