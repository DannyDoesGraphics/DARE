use dare_containers as containers;

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum ResourceState {
    /// Metadata is non-existent
    None,
    /// Metadata exists, but physical resource is unloaded
    Unloaded,
    /// Metadata exists, but physical resource is in progress of loading
    Loading,
    /// Metadata exists, but physical resource is loaded
    Loaded,
    /// Metadata exists, but physical resource is being unloaded
    ///
    /// Used to mark as in progress to be unloaded, but allows transition back to [`ResourceState::Loaded`] if needed again
    Unloading,
}

/// Per resource state
#[derive(Debug)]
pub struct ResourceEntry<T: crate::asset::traits::Asset> {
    /// Metadata associated with the virtual resource
    pub(super) metadata: T::Metadata,
    pub(super) state: Option<T::Loaded>,
}

/// Server side managing resources to be ran from a single thread (typically render thread)
#[derive(Debug)]
pub struct ResourceServer {
    //resources: containers::slot_map::SlotMap<super::handle::VirtualResource, ResourceEntry<()>>,
}
