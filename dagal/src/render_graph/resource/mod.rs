pub mod buffer;
pub(crate) mod memory;
mod physical;

use std::fmt::Debug;
use std::hash::Hash;

/// Metadata contains all information about a virtual resource to instantiate it independently
pub trait VirtualResourceMetadata: Debug + PartialEq + Eq + Hash + 'static {
    /// Physical version of the resource
    type PhysicalResource: 'static;

    /// State tracking of the resource
    type PhysicalResourceState: 'static;
}
