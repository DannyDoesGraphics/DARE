use crate::prelude::{Container, Slot};
use anyhow::Result;
use derivative::Derivative;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Weak};

pub trait DeferredDeletionSlot<T> {
    fn get_slot(&self) -> &Slot<DeferredDeletionSlotInner<T>>;
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, Hash)]
pub struct WeakDeferredDeletionSlot<T> {
    #[derivative(Hash = "ignore", Debug = "ignore", PartialEq = "ignore")]
    pub item: Weak<T>,
    pub slot: Slot<DeferredDeletionSlotInner<T>>,
}

impl<T> Clone for WeakDeferredDeletionSlot<T> {
    fn clone(&self) -> Self {
        Self {
            item: self.item.clone(),
            slot: self.slot.clone(),
        }
    }
}

impl<T> DeferredDeletionSlot<T> for WeakDeferredDeletionSlot<T> {
    fn get_slot(&self) -> &Slot<DeferredDeletionSlotInner<T>> {
        &self.slot
    }
}

impl<T> WeakDeferredDeletionSlot<T> {
    pub fn upgrade(&self) -> Option<StrongDeferredDeletionSlot<T>> {
        Some(StrongDeferredDeletionSlot {
            item: Weak::upgrade(&self.item.clone())?,
            slot: self.slot.clone(),
        })
    }
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, Eq, Hash)]
pub struct StrongDeferredDeletionSlot<T> {
    #[derivative(Hash = "ignore", Debug = "ignore", PartialEq = "ignore")]
    pub item: Arc<T>,
    pub slot: Slot<DeferredDeletionSlotInner<T>>,
}

impl<T> Clone for StrongDeferredDeletionSlot<T> {
    fn clone(&self) -> Self {
        Self {
            item: self.item.clone(),
            slot: self.slot.clone(),
        }
    }
}

impl<T> DeferredDeletionSlot<T> for StrongDeferredDeletionSlot<T> {
    fn get_slot(&self) -> &Slot<DeferredDeletionSlotInner<T>> {
        &self.slot
    }
}

impl<T> StrongDeferredDeletionSlot<T> {
    pub fn downgrade(&self) -> WeakDeferredDeletionSlot<T> {
        WeakDeferredDeletionSlot {
            slot: self.slot.clone(),
            item: Arc::downgrade(&self.item),
        }
    }
}

/// This is an inner struct which just holds the ttl of any deferred deletion entry
#[derive(Derivative)]
#[derivative(Debug)]
pub struct DeferredDeletionSlotInner<T> {
    #[derivative(Debug = "ignore")]
    entry: Arc<T>,
    /// time to live
    ttl: usize,
    /// Time of the slot
    t: usize,
}

/// [`DeferredDeletion<T, C>`] struct allows you to add slots which can be deleted if they're not
/// kept alive manually and their time goes to zero.
///
/// Uses a [`super::prelude::SpraseSlotMap`] in the background
pub struct DeferredDeletion<
    T: 'static,
    C: Container<DeferredDeletionSlotInner<T>, Slot = Slot<DeferredDeletionSlotInner<T>>>,
> {
    container: C,
    _marker: PhantomData<T>,
}

impl<
        T: 'static,
        C: Container<DeferredDeletionSlotInner<T>, Slot = Slot<DeferredDeletionSlotInner<T>>>,
    > DeferredDeletion<T, C>
{
    pub fn new() -> Self {
        Self {
            container: C::new(),
            _marker: PhantomData::<T>::default(),
        }
    }

    /// If no `t` parameter is specified, defaults to `ttl` parameter
    pub fn insert(&mut self, element: T, ttl: usize, t: Option<usize>) -> WeakDeferredDeletionSlot<T> {
        let element = Arc::new(element);
        let slot = self.container.insert(DeferredDeletionSlotInner {
            entry: element.clone(),
            ttl,
            t: t.unwrap_or(ttl),
        });
        WeakDeferredDeletionSlot {
            item: Arc::downgrade(&element),
            slot,
        }
    }

    pub fn tick(&mut self) {
        let mut slots_to_remove = Vec::new();
        {
            for entry in self.container.iter_mut() {
                if let Some(data) = entry.data {
                    data.t -= 1;
                    if data.t == 0 {
                        slots_to_remove.push(entry.slot.clone());
                    }
                }
            }
        }
        for slot in slots_to_remove {
            self.container.remove(slot).unwrap();
        }
    }

    /// Update the `t` of any deletion queue entry, if not `t` is specified, defaults to `ttl`
    pub fn update(
        &mut self,
        slot: &Slot<DeferredDeletionSlotInner<T>>,
        new_t: Option<usize>,
    ) -> Result<()> {
        self.container
            .with_slot_mut(slot, |entry| entry.t = new_t.unwrap_or(entry.ttl))?;
        Ok(())
    }
}
