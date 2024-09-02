use crate::error::ContainerErrors;
use crate::slot::Slot;
use crate::traits::Container;

struct SlotUnion<T> {
    pub slot: Slot<T>,
    pub data: Option<T>,
}


pub struct SparseSlotMap<T: 'static> {
    data: Vec<SlotUnion<T>>,
    free_list: Vec<Slot<T>>,
}

impl<T: 'static> Container<T> for SparseSlotMap<T> {
    type Slot = Slot<T>;

    fn new() -> Self {
        Self {
            data: Vec::new(),
            free_list: Vec::new(),
        }
    }

    fn insert(&mut self, element: T) -> Self::Slot {
        let next_free_slot = self.free_list.pop().unwrap_or_else(|| {
            let slot = Slot::new(self.data.len(), 0);
            self.data.push(SlotUnion {
                slot: slot.clone(),
                data: None,
            });
            slot
        });
        self.data
            .get_mut(next_free_slot.id())
            .as_mut()
            .unwrap()
            .data = Some(element);
        next_free_slot
    }

    fn is_valid(&self, slot: &Self::Slot) -> bool {
        self.data
            .get(slot.id())
            .map(|data| data.slot == *slot && data.data.is_some())
            .unwrap_or(false)
    }

    fn remove(&mut self, slot: Self::Slot) -> anyhow::Result<T> {
        self.data
            .get_mut(slot.id())
            .map(|slot_union| {
                slot_union.slot = Slot::new(slot.id(), slot.generation() + 1);
                self.free_list.push(slot_union.slot.clone());
                Ok(slot_union.data.take().unwrap())
            })
            .unwrap_or(Err(anyhow::Error::from(ContainerErrors::NonexistentSlot)))
    }

    fn total_data_len(&self) -> usize {
        self.data.len()
    }

    fn with_slot<R, F: FnOnce(&T) -> R>(&self, slot: &Self::Slot, func: F) -> anyhow::Result<R> {
        self.data
            .get(slot.id())
            .and_then(|slot_union| slot_union.data.as_ref().map(|data| Ok(func(data))))
            .unwrap_or(Err(anyhow::Error::from(ContainerErrors::NonexistentSlot)))
    }

    fn with_slot_mut<R, F: FnOnce(&mut T) -> R>(
        &mut self,
        slot: &Self::Slot,
        func: F,
    ) -> anyhow::Result<R> {
        self.data
            .get_mut(slot.id())
            .and_then(|slot_union| slot_union.data.as_mut().map(|data| Ok(func(data))))
            .unwrap_or(Err(anyhow::Error::from(ContainerErrors::NonexistentSlot)))
    }

    fn iter(&self) -> impl Iterator<Item=crate::prelude::SlotUnion<T>> {
        self.data
            .iter()
            .map(move |slot_union| crate::prelude::SlotUnion {
                slot: slot_union.slot.clone(),
                data: slot_union.data.as_ref(),
            })
    }

    fn iter_mut(&mut self) -> impl Iterator<Item=crate::prelude::SlotUnionMut<T>> {
        self.data
            .iter_mut()
            .map(move |slot_union| crate::prelude::SlotUnionMut {
                slot: slot_union.slot.clone(),
                data: slot_union.data.as_mut(),
            })
    }

    fn filter_with<F: Fn(&T) -> bool>(&mut self, predicate: F) {
        for slot_union in self.data.iter_mut() {
            if let Some(data) = slot_union.data.as_ref() {
                if !predicate(data) {
                    slot_union.data = None;
                }
            }
        }
    }
}

impl<T: 'static> Default for SparseSlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> SparseSlotMap<T> {
    pub fn with_capacity(free_list_capacity: usize, data_capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(data_capacity),
            free_list: Vec::with_capacity(free_list_capacity),
        }
    }
}
