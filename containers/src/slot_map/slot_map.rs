use crate::error::ContainerErrors;
use crate::prelude::Slot;

/// Regular slot map implementation

#[derive(Debug, PartialEq, Eq)]
pub struct SlotMap<T> {
    // usize is a reference to the proxy slot index
    data: Vec<(T, usize)>,
    slots: Vec<Slot<T>>,
    free_list: Vec<usize>,
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
        // find the next free slot
        let free_slot_index = self.free_list.pop().unwrap_or({
            self.slots.push(Slot::new(0, 0));
            self.slots.len() - 1
        });
        // push data into data vec
        let data_index = self.data.len();
        self.data.push((element, free_slot_index));

        // produce and out slot from mapping to the proxy slot
        let out_slot = self.slots.get_mut(free_slot_index).and_then(|slot| {
            // update proxy slot to data's location
            slot.id = data_index;
            // update slot out based on the proxy slot's location
            let slot_out = Slot::<T>::new(free_slot_index, slot.generation());
            Some(slot_out)
        }).map_or(Err(ContainerErrors::NonexistentSlot), |v| Ok(v))?;
        Ok(out_slot)
    }


    pub fn remove(&mut self, slot: Slot<T>) -> Result<T, ContainerErrors> {
        match self.slots.get_mut(slot.id) {
            None => Err(ContainerErrors::NonexistentSlot),
            Some(proxy_slot) => if proxy_slot.generation == slot.generation {
                // make previous outdated
                proxy_slot.generation += 1;
                // swap (if needed) data before popping
                if proxy_slot.id != self.data.len() - 1 {
                    let proxy_slot_data_index = proxy_slot.id;
                    //std::mem::swap(self.data.last_mut().unwrap(), self.data.get_mut(proxy_slot_data_index).unwrap());
                    //let mut swapped_slot
                    //let mut swapped_slot = self.slots.get_mut(slot.id).unwrap();
                    //swapped_slot.id = proxy_slot_data_index;
                }
                // to be removed must be last in data and slots
                let data = self.data.pop().unwrap();
                self.free_list.push(proxy_slot.id);
                Ok(data.0)
            } else {
                Err(ContainerErrors::GenerationMismatch)
            }
        }
    }
}