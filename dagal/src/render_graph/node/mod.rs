pub use petgraph::prelude::*;
use std::fmt::Debug;

/// Base node trait
pub trait Node: Debug {}

/// An atomic node is a node which cannot be further decomposed
pub trait AtomicNode: Node {}
/// A super node is a node which can be further decomposed into other nodes
pub trait SuperNode: Node {}
