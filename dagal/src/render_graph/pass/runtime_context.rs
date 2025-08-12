use crate::render_graph::resource::{LoadState, ResourceMetadata};
use crate::virtual_resource::VirtualResource;
use std::collections::{HashMap, HashSet};

/// Handles context for all passes in the render graph for resource access
#[derive(Debug)]
pub struct RuntimeResourceContext<'a> {
    /// Maps virtual resources to their runtime execution callbacks
    pub(crate) callbacks: &'a mut HashMap<VirtualResource, Box<dyn std::any::Any>>,

    /// All generations are expected to be 0
    pub(crate) reads: HashSet<VirtualResource>,
    /// All generations are expected to be 0
    pub(crate) writes: HashSet<VirtualResource>,
}

impl<'a> RuntimeResourceContext<'a> {
    /// Get a read access for a virtual resource by name
    ///
    /// Works for resources which are marked as read or write
    pub fn get_read<T: ResourceMetadata + 'static>(
        &self,
        virtual_resource: &VirtualResource,
    ) -> Option<&T::Physical> {
        self.reads
            .get(virtual_resource)
            .map_or_else(|| self.writes.get(virtual_resource), |vr| Some(vr)) // Get from reads first, then writes if not found
            .and_then(|vr| self.callbacks.get(vr))
            .and_then(|resource| resource.downcast_ref::<LoadState<T>>())
            .and_then(|load_state| load_state.get())
    }

    /// Get a mutable write access for a virtual resource by name
    ///
    /// Only works for write exclusive resources
    pub fn get_write<T: ResourceMetadata + 'static>(
        &mut self,
        virtual_resource: &VirtualResource,
    ) -> Option<&mut T::Physical> {
        self.writes
            .get(virtual_resource)
            .and_then(|vr| self.callbacks.get_mut(vr))
            .and_then(|resource| resource.downcast_mut::<LoadState<T>>())
            .and_then(|load_state| load_state.get_mut())
    }
}
