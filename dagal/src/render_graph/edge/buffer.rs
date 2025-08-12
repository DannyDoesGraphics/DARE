use crate::render_graph::edge::MemoryEdge;
use crate::virtual_resource::VirtualResource;

pub(crate) struct BufferEdge {
    pub resource: VirtualResource,
    pub memory: MemoryEdge,
}
