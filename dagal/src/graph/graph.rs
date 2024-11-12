use crate::graph::adj_matrix::AdjMatrix;
use crate::graph::virtual_resource::VirtualResource;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;

/// Representation of the graph itself
#[derive(Debug)]
pub struct Graph {
    /// Next index for the vertex
    next_index: AtomicU32,
    /// Adjacency matrix
    adj_matrix: AdjMatrix,
    /// vertices
    vertices: Vec<VirtualResource>,
    /// resources mappings
    resource_name_mappings: HashMap<String, VirtualResource>,
}
