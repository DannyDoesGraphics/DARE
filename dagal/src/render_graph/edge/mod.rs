use crate::render_graph::virtual_resource::VirtualResource;

mod buffer;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CompiledEdgeKind {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CompiledEdge {
    kind: CompiledEdgeKind,
    resource: VirtualResource,
}
