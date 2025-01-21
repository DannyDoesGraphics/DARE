use crate::graph::virtual_resource::{ResourceHandle, ResourceHandleUntyped, VirtualResourceEdge};
use crate::pipelines::Pipeline;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use crate::resource::traits::Resource;

#[derive(Debug)]
pub struct Pass<T: Pipeline + ?Sized> {
    /// Resources into the pass
    pub(crate) resource_in: HashSet<VirtualResourceEdge>,
    /// List of already used ids
    pub(crate) used_ids: HashMap<u32, VirtualResourceEdge>,
    /// Resources out the pass
    pub(crate) resource_out: HashSet<ResourceHandleUntyped>,
    /// Phantom
    pub(crate) _phantom: std::marker::PhantomData<T>,
}
impl<T: Pipeline + ?Sized> Default for Pass<T> {
    fn default() -> Self {
        Self {
            resource_in: HashSet::new(),
            used_ids: HashMap::new(),
            resource_out: HashSet::new(),
            _phantom: Default::default(),
        }
    }
}
impl<T: Pipeline + ?Sized> Pass<T> {

    /// Perform a read in
    pub fn read(mut self, handle: &ResourceHandleUntyped) -> Self {
        // check if input already exists
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::Read(handle.clone()));
                self.used_ids.insert(handle.id, VirtualResourceEdge::Read(handle.clone()));
            }
            Some(existing_handle) => {
                tracing::warn!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
            }
        }
        self
    }

    /// Perform a write-in
    pub fn write(mut self, mut handle: ResourceHandleUntyped) -> Self {
        // write increments gen up
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::Write(handle.clone()));
                self.used_ids.insert(handle.id, VirtualResourceEdge::Write(handle));
            }
            Some(existing_handle) => {
                panic!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
            }
        }
        self
    }

    /// Perform a read-write with the current handle
    pub fn read_write(mut self, mut handle: ResourceHandleUntyped) -> Self {
        // write increments gen up
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::ReadWrite(handle.clone()));
                self.used_ids.insert(handle.id, VirtualResourceEdge::Write(handle.clone()));
            }
            Some(existing_handle) => {
                panic!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
            }
        }
        self
    }

    /// Get the new output handle from the pass
    ///
    /// This is only necessary if you had performed write
    pub fn output_untyped(&mut self, handle: ResourceHandleUntyped) -> Option<ResourceHandleUntyped> {
        // check if resource exists in the first place
        self.used_ids.get(&handle.id).map(|handle| {
            let handle = match handle {
                VirtualResourceEdge::Read(r) => {
                    self.resource_out.insert(r.clone());
                    r.clone()
                }
                VirtualResourceEdge::Write(w) | VirtualResourceEdge::ReadWrite(w) => {
                    let mut w = w.clone();
                    w.generation += 1;
                    w
                }
            };
            self.resource_out.insert(handle.clone());
            handle
        })
    }

    pub fn output_typed<R: Resource + 'static>(&mut self, handle: ResourceHandle<R>) -> Option<ResourceHandle<R>> {
        self.output_untyped(handle.into()).map(|handle| handle.as_typed::<R>()).flatten()
    }
}