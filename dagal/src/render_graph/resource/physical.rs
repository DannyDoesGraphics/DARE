use crate::render_graph::virtual_resource::VirtualResource;
use std::collections::HashMap;

#[derive(Debug)]
struct PhysicalResourceEntry {
    state: Box<dyn std::any::Any>,
    physical: Box<dyn std::any::Any>,
}

#[derive(Debug)]
struct ResourceEntry {
    kind: std::any::TypeId,
    metadata: Option<Box<dyn std::any::Any>>,
    physical: Option<PhysicalResourceEntry>,
}

/// Stores all metadata, physical resource, and their states
/// There are 2 types of resources: Transient and Persistent
///
///
/// Transient resources are resources which are created and destroyed within a single frame and managed entirely by the
/// render graph.
///
///
/// Persistent resources are resources which persist across multiple frames and are not managed by the render graph.
pub struct PhysicalResourceStorage {
    virtual_resource_descriptions: HashMap<VirtualResource, ResourceEntry>,
}

impl PhysicalResourceStorage {
    pub(crate) fn new() -> Self {
        Self {
            virtual_resource_descriptions: HashMap::new(),
        }
    }
    /*
    TODO: update these methods to uphold resource description + state invariance without relying on complex traits
    /// Binds a virtual resource to its metadata, physical resource, and physical resource state
    ///
    /// # Panics
    /// Panics if only one of state or physical is Some, both must be Some or both None
    pub fn insert<T: VirtualResourceDescription>(
        &mut self,
        virtual_resource: VirtualResource,
        metadata: Option<T>,
        state: Option<T::PhysicalResourceState>,
        physical: Option<T::PhysicalResource>,
    ) {
        let entry = ResourceEntry {
            kind: std::any::TypeId::of::<T>(),
            metadata: metadata.map(|d| Box::new(d) as Box<dyn std::any::Any>),
            physical: if let (Some(state), Some(physical)) = (state, physical) {
                Some(PhysicalResourceEntry {
                    state: Box::new(state),
                    physical: Box::new(physical),
                })
            } else {
                None
            },
        };
        self.virtual_resource_descriptions
            .insert(virtual_resource, entry);
    }

    /// Turn a virtual resource into a persistent resource by binding its physical resource and state
    ///
    /// # Safety
    /// Ensure invariance between description if it exists as well with invariance between physical resource and state.
    /// Mismatch will cause undefined behavior and crashes.
    pub fn bind_physical<T: VirtualResourceDescription>(
        &mut self,
        virtual_resource: VirtualResource,
        state: T::PhysicalResourceState,
        physical: T::PhysicalResource,
    ) {
        if let Some(entry) = self.virtual_resource_descriptions.get_mut(&virtual_resource) {
            entry.physical = Some(PhysicalResourceEntry {
                state: Box::new(state),
                physical: Box::new(physical),
            });
        }
    }

    pub fn get<T: VirtualResourceDescription>(&mut self)
    */
}
