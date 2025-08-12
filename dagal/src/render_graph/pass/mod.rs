//! Passes define a render pass in a render graph

pub mod compute_pass;
pub mod runtime_context;

use super::node;
pub use compute_pass::*;

pub type RuntimeExecutionCallback<T> =
    Box<dyn Fn(&crate::command::CommandBuffer, &mut T) + 'static>;

/// Represents a pass in the render graph.
pub trait PassBuilder: node::traits::NodeBuilder {
    type PassRuntimeData;

    /// Execute upon a pass
    fn execute(self, execution: RuntimeExecutionCallback<Self::PassRuntimeData>) -> Self;
}
