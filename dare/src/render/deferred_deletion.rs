use std::hash::Hash;

use anyhow::Result;
use derivative::Derivative;

use dagal::util::{Slot, SparseSlotMap};

/// Represents a deletion slot
#[derive(Derivative)]
#[derivative(Debug)]
pub struct DeletionEntry<T> {
    #[derivative(Debug = "ignore")]
    pub element: T,
    ttl: usize,
    last_used: usize,
}

impl<T> DeletionEntry<T> {
    pub fn new(element: T, ttl: usize, last_used: usize) -> Self {
        Self {
            element,
            ttl,
            last_used,
        }
    }

    pub fn ttl(&self) -> usize {
        self.ttl
    }

    pub fn last_used(&self) -> usize {
        self.last_used
    }
}

#[derive(Debug)]
pub struct DeferredDeletion<T> {
    pub deferred_elements: SparseSlotMap<DeletionEntry<T>>,
    frame: usize,
}

impl<T> Default for DeferredDeletion<T> {
    fn default() -> Self {
        Self {
            deferred_elements: SparseSlotMap::new(0),
            frame: 0,
        }
    }
}

impl<T> DeferredDeletion<T> {
    pub fn insert(&mut self, element: T, ttl: usize) -> Slot<DeletionEntry<T>> {
        self.deferred_elements.insert(DeletionEntry {
            element,
            ttl,
            last_used: self.frame,
        })
    }

    pub fn update(&mut self, slot: &mut Slot<DeletionEntry<T>>) -> Result<()> {
        self.deferred_elements.with_slot_mut(slot, |slot| {
            slot.last_used = self.frame;
        })?;

        Ok(())
    }

    /// Update internal frame counter
    pub fn update_frame(&mut self, frame_increment: usize) {
        self.frame += frame_increment;
    }

    /// Clear all elements whose ttl expired
    pub fn clear_elements(&mut self) {
        self.deferred_elements.data_mut().retain(|entry| {
            if let Some(entry) = entry.data.as_ref() {
                self.frame - entry.last_used < entry.ttl
            } else {
                false
            }
        });
    }
}
