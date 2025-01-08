use crate::graph::virtual_resource::{ResourceHandleUntyped, VirtualResourceEdge};
use crate::pipelines::Pipeline;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

#[derive(Debug)]
pub struct Pass<T: Pipeline + ?Sized> {
    /// Resources into the pass
    pub(crate) resource_in: HashSet<VirtualResourceEdge>,
    /// List of already used ids
    pub(crate) used_ids: HashMap<u32, VirtualResourceEdge>,
    /// Resources out the pass
    pub(crate) resource_out: HashSet<VirtualResourceEdge>,
    /// Phantom
    _phantom: std::marker::PhantomData<T>,
}
impl<T: Pipeline> Default for Pass<T> {
    fn default() -> Self {
        Self {
            resource_in: HashSet::new(),
            used_ids: HashMap::new(),
            resource_out: HashSet::new(),
            _phantom: Default::default(),
        }
    }
}
impl<T: Pipeline> Pass<T> {

    /// Perform a read in
    pub fn read(mut self, handle: ResourceHandleUntyped) -> Self {
        // check if input already exists
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::Read(handle));
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
        handle.generation += 1;
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::Write(handle));
            }
            Some(existing_handle) => {
                if !existing_handle.write() {
                    panic!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
                } else {
                    tracing::warn!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
                }
            }
        }
        self
    }

    /// Perform a read-write with the current handle
    pub fn read_write(mut self, mut handle: ResourceHandleUntyped) -> Self {
        // write increments gen up
        handle.generation += 1;
        match self.used_ids.get(&handle.id) {
            None => {
                self.resource_in.insert(VirtualResourceEdge::ReadWrite(handle));
            }
            Some(existing_handle) => {
                if !existing_handle.write() {
                    panic!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
                } else {
                    tracing::warn!("Tried inserting handle, {:?}, found existing handle in pass {:?}", handle, existing_handle);
                }
            }
        }
        self
    }

    /// Get the new output handle from the pass
    ///
    /// This is only necessary if you had performed write
    pub fn output(&self, handle: &ResourceHandleUntyped) -> Option<ResourceHandleUntyped> {
        self.used_ids.get(&handle.id).map(|handle| {
            handle.deref().clone()
        })
    }
}