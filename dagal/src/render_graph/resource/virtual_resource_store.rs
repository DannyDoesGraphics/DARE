use std::any::Any;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
struct InternalSlot {
    id: u32,
    generation: u32,
}

/// A collection for storing [`super::VirtuableResource`] types, realizing, and handling virtualization
#[derive(Debug, Default)]
pub struct VirtualResourceStorage {
    free_list: Vec<usize>,
    inner: Vec<InternalSlot>,
    items: Vec<InnerBox>,
}

#[derive(Debug)]
pub struct Physical<T: super::VirtualableResource> {
    pub physical: T::Physical,
    pub state: T::PhysicalStore,
}
#[derive(Debug)]
pub(crate) struct InnerItem<T: super::VirtualableResource> {
    description: T::Description,
    physical: Option<Physical<T>>,
}

#[derive(Debug)]
pub(crate) struct InnerBox {
    pub(crate) back_index: usize,
    pub(crate) item: Box<dyn Any>,
}

impl VirtualResourceStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: super::VirtualableResource>(
        &mut self,
        description: T::Description,
    ) -> super::ResourceId {
        let internal_idx: usize = self.items.len();
        let inner_slot_idx = self.free_list.pop().unwrap_or({
            let idx = self.inner.len();
            self.inner.push(InternalSlot {
                id: 0,
                generation: 0,
            });
            idx
        });
        self.items.push(InnerBox {
            item: Box::new(InnerItem::<T> {
                description,
                physical: None,
            }),
            back_index: inner_slot_idx,
        });

        let inner_slot: &mut InternalSlot = self.inner.get_mut(inner_slot_idx).unwrap();
        inner_slot.id = internal_idx as u32;
        super::ResourceId::new(inner_slot_idx as u32, inner_slot.generation)
    }

    pub(crate) fn get_item<T: super::VirtualableResource>(
        &self,
        id: super::ResourceId,
    ) -> Option<&InnerBox> {
        let inner: &InternalSlot = self.inner.get(id.id() as usize)?;
        if inner.generation != id.generation() {
            return None;
        }
        let item = self.items.get(inner.id as usize);
        if item.is_some() && !item.unwrap().item.is::<InnerItem<T>>() {
            None
        } else {
            item
        }
    }

    pub fn get_description<T: super::VirtualableResource>(
        &self,
        id: super::ResourceId,
    ) -> Option<&T::Description> {
        self.get_item::<T>(id)
            .and_then(|i| i.item.downcast_ref::<InnerItem<T>>())
            .map(|v| &v.description)
    }

    pub(crate) fn get_physical<T: super::VirtualableResource>(
        &self,
        id: super::ResourceId,
    ) -> Option<&Physical<T>> {
        self.get_item::<T>(id)
            .and_then(|i| i.item.downcast_ref::<InnerItem<T>>())
            .and_then(|v| v.physical.as_ref())
    }

    /// Returns [`Some`] if item was successfully inserted
    pub fn insert_physical<T: super::VirtualableResource>(
        &mut self,
        id: super::ResourceId,
        physical: T::Physical,
        state: T::PhysicalStore,
    ) -> Option<()> {
        self.mut_inspect_item(id, |item: &mut InnerItem<T>| {
            item.physical.replace(Physical { physical, state });
        })
        .map(|_| Some(()))
        .unwrap_or(None)
    }

    pub fn get_physical_store<T: super::VirtualableResource>(
        &self,
        id: super::ResourceId,
    ) -> Option<&T::Physical> {
        self.get_item::<T>(id)
            .and_then(|i| i.item.downcast_ref::<InnerItem<T>>())
            .and_then(|i| i.physical.as_ref())
            .map(|p| &p.physical)
    }

    pub fn get_physical_state<T: super::VirtualableResource>(
        &self,
        id: super::ResourceId,
    ) -> Option<&T::PhysicalStore> {
        self.get_item::<T>(id)
            .and_then(|i| i.item.downcast_ref::<InnerItem<T>>())
            .and_then(|i| i.physical.as_ref())
            .map(|p| &p.state)
    }

    /// Returns [`None`] if item does not exist
    pub(crate) fn mut_inspect_item<
        T: super::VirtualableResource,
        O,
        F: FnOnce(&mut InnerItem<T>) -> O,
    >(
        &mut self,
        id: super::ResourceId,
        f: F,
    ) -> Option<O> {
        let inner: &mut InternalSlot = self.inner.get_mut(id.id() as usize)?;
        let item = self.items.get_mut(inner.id as usize)?;
        let dc = item.item.downcast_mut::<InnerItem<T>>()?;
        Some(f(dc))
    }

    /// Forcefuully remove a virtual resource
    pub(crate) fn remove<T: super::VirtualableResource>(
        &mut self,
        id: super::ResourceId,
    ) -> Option<Box<InnerItem<T>>> {
        // check if we grab it first
        self.get_item::<T>(id)?;
        let inner: InternalSlot = *self.inner.get(id.id() as usize).unwrap();
        let last_idx: usize = self.items.len() - 1;
        if inner.id as usize != last_idx {
            // swap required item with last item
            self.items.swap(inner.id as usize, last_idx);

            // update the inner slot
            self.inner
                .get_mut(self.items.get(inner.id as usize).unwrap().back_index)
                .unwrap()
                .id = inner.id;
            // free up inner slot
            self.inner.get_mut(id.id() as usize).unwrap().generation += 1;
            self.free_list.push(id.id() as usize);
        }
        self.items
            .pop()
            .and_then(|i| i.item.downcast::<InnerItem<T>>().ok())
    }
}

#[cfg(test)]
mod tests {

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    struct Test(i32);
    impl std::ops::Deref for Test {
        type Target = i32;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl From<i32> for Test {
        fn from(value: i32) -> Self {
            Self(value)
        }
    }
    impl super::super::VirtualableResource for Test {
        type Description = i32;
        type Physical = i32;
        type PhysicalStore = i32;
    }

    use super::super::ResourceId;
    use super::*;
    #[test]
    fn insert_into_slot_map() {
        let mut s: VirtualResourceStorage = VirtualResourceStorage::new();
        let a: ResourceId = s.insert::<Test>(1i32);
        let b: ResourceId = s.insert::<Test>(2i32);

        assert_eq!(*s.get_description::<Test>(a).unwrap(), 1i32);
        assert_eq!(*s.get_description::<Test>(b).unwrap(), 2i32);
    }

    #[test]
    fn remove_slot_map() {
        let mut s: VirtualResourceStorage = VirtualResourceStorage::new();
        let a: ResourceId = s.insert::<Test>(1i32);
        let b: ResourceId = s.insert::<Test>(2i32);
        let c: ResourceId = s.insert::<Test>(3i32);

        assert_eq!(s.remove::<Test>(b).unwrap().description, 2i32);
        assert_eq!(*s.get_description::<Test>(a).unwrap(), 1i32);
        assert_eq!(*s.get_description::<Test>(c).unwrap(), 3i32);
        assert_eq!(s.get_description::<Test>(b), None);
    }

    #[test]
    fn remove_insert_map() {
        let mut s: VirtualResourceStorage = VirtualResourceStorage::new();
        let a: ResourceId = s.insert::<Test>(1i32);
        let b: ResourceId = s.insert::<Test>(2i32);
        let c: ResourceId = s.insert::<Test>(3i32);

        assert_eq!(s.remove::<Test>(b).unwrap().description, 2i32);
        assert_eq!(*s.get_description::<Test>(a).unwrap(), 1i32);
        assert_eq!(*s.get_description::<Test>(c).unwrap(), 3i32);
        let d: ResourceId = s.insert::<Test>(-1i32);
        assert_eq!(*s.get_description::<Test>(d).unwrap(), -1i32);
        assert_eq!(*s.get_description::<Test>(a).unwrap(), 1i32);
        assert_eq!(s.get_description::<Test>(b), None);
        assert_eq!(*s.get_description::<Test>(c).unwrap(), 3i32);
    }
}
