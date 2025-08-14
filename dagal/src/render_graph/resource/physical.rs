use crate::render_graph::resource::VirtualResourceMetadata;
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
pub struct PhysicalResourceStorage {
    virtual_resource_metadata: HashMap<VirtualResource, ResourceEntry>,
}

impl PhysicalResourceStorage {
    pub(crate) fn create() -> Self {
        Self {
            virtual_resource_metadata: HashMap::new(),
        }
    }

    /// Binds a virtual resource to its metadata, physical resource, and physical resource state
    ///
    /// # Panics
    /// Panics if only one of state or physical is Some, both must be Some or both None
    pub fn insert<T: VirtualResourceMetadata>(
        &mut self,
        virtual_resource: VirtualResource,
        metadata: Option<T>,
        state: Option<T::PhysicalResourceState>,
        physical: Option<T::PhysicalResource>,
    ) {
        assert_eq!(state.is_some(), physical.is_some());
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
        self.virtual_resource_metadata
            .insert(virtual_resource, entry);
    }

    /// Bind a physical resource directly to a virtual resource and it's associated metadata
    ///
    /// # Safety
    /// Ensure invariance between metadata if it exists as well with invariance between physical resource and state.
    /// Mismatch will cause undefined behavior and crashes.
    pub fn bind_physical<T: VirtualResourceMetadata>(
        &mut self,
        virtual_resource: VirtualResource,
        state: T::PhysicalResourceState,
        physical: T::PhysicalResource,
    ) {
        if let Some(entry) = self.virtual_resource_metadata.get_mut(&virtual_resource) {
            entry.physical = Some(PhysicalResourceEntry {
                state: Box::new(state),
                physical: Box::new(physical),
            });
        }
    }

    
}
