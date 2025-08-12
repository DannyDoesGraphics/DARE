//! The render graph is a simple DAG executor which handles resource aliasing, and synchronization

mod compiled_graph;
pub mod edge;
pub mod graph;
pub mod node;
pub mod pass;
pub mod resource;
