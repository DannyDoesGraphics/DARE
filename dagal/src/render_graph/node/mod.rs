use std::fmt::Debug;

/// Base node trait
pub trait Node: Debug {}

/// An atomic node is a node which cannot be further decomposed
/// Used purely for type distinction, has no real functionality
pub trait AtomicNode: Node {}

/// A super node is a node which can be further decomposed into other nodes
pub trait SuperNode: Node {
    type DecomposeInfo;

    /// Decomposes the super node into other nodes
    fn decompose(self, decompose_info: Self::DecomposeInfo) -> Vec<Box<dyn Node>>;
}
