//! # [`Node`]
//! Represents a node within the [`super::super::graph::RenderGraph`]. There are only three types of
//! nodes: [`AtomicNode`], [`SuperNode`], and a [`Node`]. An atomic node cannot be decomposed
//! further, while a super node is a collection of any nodes (atomic or not).
//!
//! A regular [`Node`] does not care for decomposition or atomic logic, but fundamentally behaves as
//! [`AtomicNode`]

use crate::virtual_resource::VirtualResource;
use std::fmt::Debug;

/// Trait for a single atomic node in a render graph.
/// Cannot be decomposed further in contrast to [`SuperNode`]
pub(crate) trait AtomicNode: Node {}

/// A super node a simple collection of any nodes
pub(crate) trait SuperNode: Node {
    /// Decompose a super node into its element nodes
    fn decompose(self) -> Vec<Box<dyn AtomicNode>>;
}

/// A base node to define reads/writes in a render graph
pub(crate) trait Node: Debug {
    /// Get resource reads of the node
    fn reads(&self) -> &[VirtualResource];

    /// Get resource writes of the node
    fn writes(&self) -> &[VirtualResource];
}

/// Responsible for building nodes in the render graph
pub(crate) trait NodeBuilder: Debug {
    /// Node type that is being built
    type Node: Node;

    /// Submit the node to the render graph
    fn submit(self);
}
