//! Responsible for containing a collection of resources

use std::any::Any;

use crate::resource::traits::{Buildable, Resource};

use super::{UntypedNoGenerationVirtualResource, VirtualResource};

/// Internal slot map
struct Slot {
    generation: u32,
    data_index: u32,
}

/// Internal slot map used to O(1) accesses without relying on hashing
struct SlotMap<A> {
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    data: Vec<A>,
    data_slot: Vec<u32>,
}

impl<A> Default for SlotMap<A> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
            data: Vec::new(),
            data_slot: Vec::new(),
        }
    }
}

impl<A> SlotMap<A> {
    pub(crate) fn insert(&mut self, value: A) -> (u32, u32) {
        let data_index = self.data.len() as u32;
        self.data.push(value);
        let id = if let Some(free) = self.free_slots.pop() {
            self.slots[free as usize].data_index = data_index;
            free
        } else {
            self.slots.push(Slot {
                generation: 0,
                data_index,
            });
            (self.slots.len() - 1) as u32
        };
        self.data_slot.push(id);
        (id, self.slots[id as usize].generation)
    }

    pub fn get_without_generation(&self, id: u32) -> Option<&A> {
        self.data
            .get((self.slots.get(id as usize)?).data_index as usize)
    }

    pub fn get_mut_without_generation(&mut self, id: u32) -> Option<&mut A> {
        let data_index = self.slots.get(id as usize)?.data_index;
        self.data.get_mut(data_index as usize)
    }

    pub fn get(&self, id: u32, generation: u32) -> Option<&A> {
        let slot = self.slots.get(id as usize)?;
        if slot.generation != generation {
            return None;
        }
        self.data.get(slot.data_index as usize)
    }

    pub fn get_mut(&mut self, id: u32, generation: u32) -> Option<&mut A> {
        let slot = self.slots.get(id as usize)?;
        if slot.generation != generation {
            return None;
        }
        let data_index = slot.data_index;
        self.data.get_mut(data_index as usize)
    }

    pub(crate) fn remove(&mut self, id: u32, generation: u32) -> Option<A> {
        let data_index = {
            let slot = self.slots.get(id as usize)?;
            if slot.generation != generation {
                return None;
            }
            slot.data_index
        };

        self.slots[id as usize].generation = self.slots[id as usize].generation.wrapping_add(1);
        self.free_slots.push(id);

        let last_index = (self.data.len() - 1) as u32;
        self.data.swap(data_index as usize, last_index as usize);
        self.data_slot
            .swap(data_index as usize, last_index as usize);
        let removed = self.data.pop().unwrap();
        self.data_slot.pop();

        if data_index != last_index {
            let moved_id = self.data_slot[data_index as usize];
            self.slots[moved_id as usize].data_index = data_index;
        }

        Some(removed)
    }
}

enum ResourceSlot<'a> {
    Imported(&'a mut dyn Any),
    Transient {
        description: Box<dyn Any>,
        realized: Option<Box<dyn Any>>,
    },
}

#[derive(Default)]
pub struct VirtualResourceCollection<'a> {
    slots: SlotMap<ResourceSlot<'a>>,
}

impl<'a> VirtualResourceCollection<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub(crate) fn create<A: Buildable + 'static>(
        &mut self,
        description: A::Desc,
    ) -> VirtualResource<A> {
        let (id, generation) = self.slots.insert(ResourceSlot::Transient {
            description: Box::new(description),
            realized: None,
        });
        VirtualResource::new(id, generation)
    }

    pub(crate) fn import<A: Resource + 'static>(
        &mut self,
        resource: &'a mut A,
    ) -> VirtualResource<A> {
        let (id, generation) = self.slots.insert(ResourceSlot::Imported(resource));
        VirtualResource::new(id, generation)
    }

    #[must_use]
    pub fn get<A: Buildable + 'static>(
        &mut self,
        handle: &super::VirtualResource<A>,
        device: &crate::device::LogicalDevice,
        allocator: &'a A::Alloc,
    ) -> Option<&A> {
        match self.slots.get_mut(handle.id, handle.generation)? {
            ResourceSlot::Imported(resource) => resource.downcast_ref::<A>(),
            ResourceSlot::Transient {
                description,
                realized,
            } => {
                if realized.is_none() {
                    let description = description.downcast_ref::<A::Desc>()?;
                    let built: A = A::build(description, device, allocator).ok()?;
                    *realized = Some(Box::new(built));
                }
                realized.as_ref()?.downcast_ref::<A>()
            }
        }
    }

    #[must_use]
    pub fn get_mut<A: Buildable + 'static>(
        &mut self,
        handle: &super::VirtualResource<A>,
        device: &crate::device::LogicalDevice,
        allocator: &'a A::Alloc,
    ) -> Option<&mut A> {
        match self.slots.get_mut(handle.id, handle.generation)? {
            ResourceSlot::Imported(resource) => resource.downcast_mut::<A>(),
            ResourceSlot::Transient {
                description,
                realized,
            } => {
                if realized.is_none() {
                    let description = description.downcast_ref::<A::Desc>()?;
                    let built: A = A::build(description, device, allocator).ok()?;
                    *realized = Some(Box::new(built));
                }
                realized.as_mut()?.downcast_mut::<A>()
            }
        }
    }

    /// Removes a realized resource
    #[allow(dead_code)]
    pub(crate) fn remove_realized<A: Buildable + 'static>(
        &mut self,
        handle: UntypedNoGenerationVirtualResource,
    ) -> Option<A> {
        match self.slots.get_mut_without_generation(handle.id)? {
            ResourceSlot::Imported(_) => None,
            ResourceSlot::Transient { realized, .. } => {
                if realized.as_ref()?.is::<A>() {
                    realized.take()?.downcast::<A>().ok().map(|built| *built)
                } else {
                    None
                }
            }
        }
    }
}
