use crate::{physical_resource::handle::VirtualResource, util::either::Either};

/// Functions as a command bus to allow outside threads to write
#[derive(Debug)]
pub enum Commit {
    /// Register metadata passing in metadata
    Register(Box<dyn std::any::Any>),
    /// Unregister metadata
    Unregister(Either<Box<dyn std::any::Any>, VirtualResource>),
    /// Queue to be unloaded
    Unload(Either<Box<dyn std::any::Any>, VirtualResource>),
    /// Request to be loaded
    Load(Either<Box<dyn std::any::Any>, VirtualResource>),
    /// If metadata does not exist, load. After loaded, try to load it
    InsertAndLoad(Either<Box<dyn std::any::Any>, VirtualResource>)
}

/// Physical resource client which can be distributed across threads
pub struct PhysicalResourceClient {
    pub(super) commit_send: crossbeam_channel::Sender<Commit>,
}